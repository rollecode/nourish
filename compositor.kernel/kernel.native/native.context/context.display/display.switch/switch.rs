//! Live ACTIVE-OUTPUT switch from the settings window with a user-confirmed fault
//! gate. Single-output scanout: only one pipe is lit at a time (bringing a second
//! one up alongside fails the atomic modeset), so a switch TEARS DOWN the current
//! output first, then brings the target up as the sole output (reusing the smithay
//! `Output`, sized to the target mode) — the same shape as startup, which the
//! target is already known-good for. A revert REBUILDS the original connector.
//! Sibling of `display.mode`; driven via OUTPUT_SWITCH_REQUEST.
//!
//! The actual teardown+modeset MUST NOT run inside the vblank/render callback, so
//! `drain` (which is called from the input loop and the render path) only defers
//! the work onto a one-shot `Timer::immediate()` loop source — the modeset then
//! runs in its own event-loop dispatch next iteration, exactly like the
//! VT-switch/session-resume path.
use compositor_kernel_native_context_render_base::render::{NativeRenderContext, OutputSwitchBaseline};
use compositor_kernel_graphic_preference_output_profile::profile::{self, ModeRequest};
use compositor_orchestration_event_output_base::output::OutputChange;
use compositor_orchestration_core_state_base::Loop;
use compositor_orchestration_driver_lid_base::base::{DISPLAY_OFF_MUT, DISPLAY_SNAPSHOT_MUT};
use compositor_orchestration_driver_output_base::base::{
    ApplyResult, ModeInfo, OutputModesSnapshot, OutputSwitchRequest, OutputsSnapshot, OUTPUTS_SNAPSHOT_MUT,
    OUTPUT_MODES_SNAPSHOT_MUT, OUTPUT_SWITCH_REQUEST_MUT, OUTPUT_SWITCH_RESULT_MUT,
};
use smithay::backend::drm::DrmDevice;
use smithay::output::Mode;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::RegistrationToken;
use smithay::reexports::drm::control::{connector, Mode as DrmMode};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

/// How long a provisionally-switched output survives without an explicit Keep
/// before auto-reverting. Armed on the calloop loop handle (NOT a per-frame
/// counter — frames may halt on the new output, but the timer still fires and
/// recovers the screen).
const CONFIRM_TIMEOUT: Duration = Duration::from_secs(15);
type Ctx = Rc<RefCell<NativeRenderContext>>;

fn set_result(state: &mut Loop, r: ApplyResult) {
    *state.inner.kernel.get_mut(&OUTPUT_SWITCH_RESULT_MUT) = Some(r);
}

fn mode_info(m: DrmMode) -> ModeInfo {
    ModeInfo { width: m.size().0, height: m.size().1, refresh_mhz: m.vrefresh() * 1000 }
}

/// The EDID identity key ("make model serial") for a connector — the same key the
/// picker selects with and the settings-editor persists.
fn identity_key(drm: &DrmDevice, info: &connector::Info) -> String {
    let raw = compositor_kernel_drm_edid_parse_base::parse::read(drm, info);
    let parsed = raw.as_ref().and_then(compositor_kernel_drm_edid_parse_base::parse::parse);
    compositor_kernel_drm_edid_identity_base::identity::identity(parsed.as_ref()).key()
}

/// The connected connector whose EDID identity matches `key`.
fn find_target(drm: &DrmDevice, key: &str) -> Option<connector::Info> {
    let res = compositor_kernel_drm_connector_scan_base::scan::resources(drm);
    let infos = compositor_kernel_drm_connector_scan_base::scan::connectors(drm, &res);
    infos
        .into_iter()
        .find(|i| i.state() == connector::State::Connected && identity_key(drm, i) == key)
}

/// The EDID identity of the connector currently driving the compositor (by handle).
fn current_connector_name(ctx: &NativeRenderContext) -> Option<String> {
    let mgr = ctx.drm_output_manager.borrow();
    let drm = mgr.device();
    let res = compositor_kernel_drm_connector_scan_base::scan::resources(drm);
    let infos = compositor_kernel_drm_connector_scan_base::scan::connectors(drm, &res);
    infos.iter().find(|i| i.handle() == ctx.connector).map(|i| identity_key(drm, i))
}

/// Tear down the current pipe (freeing its CRTC) and bring `target` up as the sole
/// output, reusing the smithay `Output`. On success the context reflects the new
/// connector/mode. On failure `ctx.drm_output` is left `None` — the caller must
/// rebuild a working output (render frames skip while it is `None`).
fn bring_up(ctx: &mut NativeRenderContext, target: &connector::Info, requested: Option<ModeInfo>) -> Result<(), String> {
    // Drop the current output FIRST so its CRTC/bandwidth is free for the target
    // (the atomic modeset of a second simultaneous pipe is rejected).
    ctx.drm_output = None;
    let built = compositor_kernel_native_context_display_build::build::build(
        &ctx.drm_output_manager,
        &ctx.gpu_binding,
        &ctx.output,
        &[],
        target,
        requested,
    )?;
    let env = compositor_developer_environment_config_base::base::get();
    let new_hdr_active = env.hdr && built.hdr.hdr_capable() && ctx.vulkan_mode;
    let new_mode = Mode::from(built.drm_mode);
    ctx.drm_output = Some(built.drm_output);
    ctx.mode = new_mode;
    ctx.current_drm_mode = built.drm_mode;
    ctx.modes = built.modes;
    ctx.connector = built.connector;
    ctx.hdr_caps = built.hdr;
    ctx.hdr_active = new_hdr_active;
    ctx.hdr_signalled = false;
    ctx.output.change_current_state(Some(new_mode), None, None, None);
    Ok(())
}

/// Rebuild the baseline connector (revert). Best-effort: if its connector vanished
/// the screen may be left dark, which is logged.
fn revert_to(ctx: &mut NativeRenderContext, b: &OutputSwitchBaseline) -> Result<(), String> {
    let target = {
        let mgr = ctx.drm_output_manager.borrow();
        find_target(mgr.device(), &b.connector_name)
    };
    let target = target.ok_or_else(|| format!("revert target {:?} not connected", b.connector_name))?;
    bring_up(ctx, &target, Some(b.mode))
}

/// Rewrite the rim-facing snapshots (full connector list + active modes + lid) for
/// the connector now driving the compositor.
fn write_snapshots(state: &mut Loop, ctx: &NativeRenderContext) {
    let active = ctx.connector;
    let active_mode = mode_info(ctx.current_drm_mode);
    let snap = {
        let mgr = ctx.drm_output_manager.borrow();
        let drm = mgr.device();
        let snap = compositor_kernel_native_context_display_enumerate::enumerate::enumerate(drm, active, active_mode);
        let display_snap = compositor_kernel_native_context_display_base::base::compute(drm, active);
        *state.inner.kernel.get_mut(&DISPLAY_SNAPSHOT_MUT) = display_snap;
        snap
    };
    if let Some(d) = snap.displays.iter().find(|d| d.active) {
        *state.inner.kernel.get_mut(&OUTPUT_MODES_SNAPSHOT_MUT) =
            OutputModesSnapshot { edid_key: d.edid_key.clone(), current: d.current, available: d.available.clone() };
    }
    *state.inner.kernel.get_mut(&OUTPUTS_SNAPSHOT_MUT) = snap;
}

/// Take a pending request (if any) and DEFER it onto a one-shot loop timer, so the
/// modeset never runs inside the vblank/render callback that may be calling this.
pub fn drain(state: &mut Loop, ctx_rc: &Ctx) {
    let Some(req) = state.inner.kernel.get_mut(&OUTPUT_SWITCH_REQUEST_MUT).take() else { return };
    let ctx = ctx_rc.clone();
    state
        .loop_handle
        .insert_source(Timer::immediate(), move |_, _, state: &mut Loop| {
            match &req {
                OutputSwitchRequest::Apply { edid_key, mode } => apply(state, &ctx, edid_key.clone(), *mode),
                OutputSwitchRequest::Confirm => finish(state, &ctx, false),
                OutputSwitchRequest::Revert => finish(state, &ctx, true),
            }
            TimeoutAction::Drop
        })
        .expect("output switch deferral timer registration failed");
}

/// The actual switch: tear the current output down and bring the target up. Runs
/// only from the deferral timer's callback (never inside the vblank/render path).
fn apply(state: &mut Loop, ctx_rc: &Ctx, edid_key: String, requested: Option<ModeInfo>) {
    let mut ctx = ctx_rc.borrow_mut();
    // Re-apply from the ORIGINAL baseline: restore the original first so this
    // switch's baseline is the true original (mirrors the mode gate).
    if let Some(b) = ctx.output_revert.take() {
        state.loop_handle.remove(b.timer);
        if let Err(e) = revert_to(&mut ctx, &b) {
            warn!("could not restore original before re-apply: {e}");
        }
    }
    // Capture the current (soon-to-be-previous) connector + mode for revert.
    let prev_name = current_connector_name(&ctx);
    let prev_mode = mode_info(ctx.current_drm_mode);

    let target = {
        let mgr = ctx.drm_output_manager.borrow();
        find_target(mgr.device(), &edid_key)
    };
    let Some(target) = target else {
        drop(ctx);
        warn!("switch target {edid_key:?} not connected");
        return set_result(state, ApplyResult::Failed);
    };

    if let Err(e) = bring_up(&mut ctx, &target, requested) {
        warn!("output switch build failed: {e}; restoring previous output");
        if let Some(prev) = prev_name.as_deref() {
            let prev_info = {
                let mgr = ctx.drm_output_manager.borrow();
                find_target(mgr.device(), prev)
            };
            if let Some(prev_info) = prev_info {
                if let Err(e2) = bring_up(&mut ctx, &prev_info, Some(prev_mode)) {
                    abort!("failed to restore previous output after a failed switch: {e2}");
                }
            }
        }
        drop(ctx);
        return set_result(state, ApplyResult::Failed);
    }

    let token = arm(state, ctx_rc);
    ctx.output_revert = prev_name.map(|connector_name| OutputSwitchBaseline {
        connector_name,
        mode: prev_mode,
        timer: token,
    });
    write_snapshots(state, &ctx);
    drop(ctx);
    state.schedule_redraw();
    info!(
        "output switch: now driving connector {edid_key:?}; auto-revert in {}s unless kept",
        CONFIRM_TIMEOUT.as_secs()
    );
    set_result(state, ApplyResult::Provisional);
}

/// `revert=false` keeps the new output (Confirm: the previous one is already gone);
/// `true` rebuilds the previous output (Revert).
fn finish(state: &mut Loop, ctx_rc: &Ctx, revert: bool) {
    let mut ctx = ctx_rc.borrow_mut();
    let Some(b) = ctx.output_revert.take() else { return };
    state.loop_handle.remove(b.timer);
    if revert {
        if let Err(e) = revert_to(&mut ctx, &b) {
            warn!("output switch revert failed: {e}");
        }
        write_snapshots(state, &*ctx);
        drop(ctx);
        state.schedule_redraw();
        set_result(state, ApplyResult::Reverted);
    } else {
        drop(ctx);
        set_result(state, ApplyResult::Confirmed);
    }
}

/// Arm the one-shot revert watchdog; on fire it rebuilds whatever baseline is pending.
fn arm(state: &mut Loop, ctx_rc: &Ctx) -> RegistrationToken {
    let ctx_rc = ctx_rc.clone();
    state
        .loop_handle
        .insert_source(Timer::from_duration(CONFIRM_TIMEOUT), move |_, _, state: &mut Loop| {
            let mut ctx = ctx_rc.borrow_mut();
            if let Some(b) = ctx.output_revert.take() {
                info!("output switch: {}s elapsed — auto-reverting to {:?}", CONFIRM_TIMEOUT.as_secs(), b.connector_name);
                if let Err(e) = revert_to(&mut ctx, &b) {
                    warn!("output switch auto-revert failed: {e}");
                }
                write_snapshots(state, &*ctx);
                drop(ctx);
                state.schedule_redraw();
                set_result(state, ApplyResult::Reverted);
            }
            TimeoutAction::Drop
        })
        .expect("output switch revert watchdog registration failed")
}

/// The preferred-monitor key (the FIRST output profile's identity — same default-output
/// rule startup uses), if set.
fn preferred_key() -> Option<String> {
    profile::get().into_iter().next().and_then(|p| p.identity)
}

/// Resolve the mode to bring `info` up at FROM PREFERENCES — its per-output profile's
/// advertised mode, else the global default mode — mirroring startup. `None` lets the
/// builder fall back to the default-policy mode.
fn pref_mode(drm: &DrmDevice, info: &connector::Info) -> Option<ModeInfo> {
    let to_info = |m: &ModeRequest| match m {
        ModeRequest::Advertised { width, height, refresh_mhz } => {
            Some(ModeInfo { width: *width, height: *height, refresh_mhz: *refresh_mhz })
        }
        _ => None,
    };
    let key = identity_key(drm, info);
    let profiles = profile::get();
    profiles
        .iter()
        .find(|p| p.identity.as_deref() == Some(key.as_str()))
        .or_else(|| profiles.iter().find(|p| p.identity.is_none()))
        .and_then(|p| p.mode.as_ref())
        .and_then(to_info)
        .or_else(|| profile::default_mode().as_ref().and_then(to_info))
}

/// Pick the connector to drive among the connected ones: the preferred monitor if
/// present, else the first connected — exactly the startup `connector.select` policy.
fn pick_target(drm: &DrmDevice, connected: &[connector::Info]) -> Option<connector::Info> {
    if let Some(key) = preferred_key() {
        if let Some(c) = connected.iter().find(|c| identity_key(drm, c) == key) {
            return Some(c.clone());
        }
    }
    connected.first().cloned()
}

/// Tear the display down and idle the render loop until a monitor returns.
fn go_dark(state: &mut Loop, ctx: &mut NativeRenderContext) {
    ctx.drm_output = None;
    *state.inner.kernel.get_mut(&DISPLAY_OFF_MUT) = true;
    *state.inner.kernel.get_mut(&OUTPUT_MODES_SNAPSHOT_MUT) = OutputModesSnapshot::default();
    *state.inner.kernel.get_mut(&OUTPUTS_SNAPSHOT_MUT) = OutputsSnapshot::default();
}

/// Hotplug reconciliation: ensure the compositor drives the best connected output
/// (preferred monitor, mode from preferences — the same flow as startup), or goes
/// dark and waits when none is connected. Idempotent: a no-op while the current
/// active output is still connected. Called from the udev hotplug path (its own
/// loop dispatch, never the vblank callback), not user-confirmed — no revert gate.
pub fn reconcile(state: &mut Loop, ctx_rc: &Ctx) -> Option<OutputChange> {
    let mut ctx = ctx_rc.borrow_mut();
    // A topology change supersedes any pending user provisional switch.
    if let Some(b) = ctx.output_revert.take() {
        state.loop_handle.remove(b.timer);
    }
    let was_dark = ctx.drm_output.is_none();
    let connected = {
        let mgr = ctx.drm_output_manager.borrow();
        let drm = mgr.device();
        let res = compositor_kernel_drm_connector_scan_base::scan::resources(drm);
        let infos = compositor_kernel_drm_connector_scan_base::scan::connectors(drm, &res);
        infos.into_iter().filter(|i| i.state() == connector::State::Connected).collect::<Vec<_>>()
    };
    // The DESIRED output, recomputed on EVERY topology change exactly like startup:
    // the preferred monitor (`outputs[0]` identity) if it is connected, else the
    // first connected. This is what makes a reconnected preferred monitor reclaim
    // the display (not just failover when the active one vanishes).
    let target = {
        let mgr = ctx.drm_output_manager.borrow();
        pick_target(mgr.device(), &connected)
    };
    let already_desired =
        ctx.drm_output.is_some() && target.as_ref().map(|t| t.handle()) == Some(ctx.connector);
    info!(
        "reconcile: {} connected, currently_dark={}, already_driving_preferred={}",
        connected.len(),
        was_dark,
        already_desired
    );
    // Already driving the desired (preferred) output → no switch, just refresh the
    // connected-monitor list so the picker stays current.
    if already_desired {
        write_snapshots(state, &ctx);
        return Some(OutputChange::Changed);
    }
    let Some(target) = target else {
        go_dark(state, &mut ctx);
        warn!("no monitor connected — display dark, awaiting hotplug");
        return Some(OutputChange::WentDark);
    };
    // Drive the desired output: preferred monitor reconnected, failover to another
    // monitor, or recover from dark — all the same "select + bring up" as startup.
    let requested = {
        let mgr = ctx.drm_output_manager.borrow();
        pref_mode(mgr.device(), &target)
    };
    match bring_up(&mut ctx, &target, requested) {
        Ok(()) => {
            *state.inner.kernel.get_mut(&DISPLAY_OFF_MUT) = false;
            write_snapshots(state, &ctx);
            drop(ctx);
            state.schedule_redraw();
            info!("display reconcile: now driving the preferred connected output");
            Some(if was_dark { OutputChange::Recovered } else { OutputChange::Changed })
        }
        Err(e) => {
            warn!("reconcile bring-up failed: {e}; going dark");
            go_dark(state, &mut ctx);
            Some(OutputChange::WentDark)
        }
    }
}
