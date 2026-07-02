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
use smithay::reexports::drm::control::{connector, crtc, Mode as DrmMode};
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
    compositor_kernel_drm_edid_identity_base::identity::identity(
        parsed.as_ref(),
        &format!("{:?}-{}", info.interface(), info.interface_id()),
    )
    .key()
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
    infos.iter().find(|i| i.handle() == ctx.pipe().connector).map(|i| identity_key(drm, i))
}

/// Tear down the current pipe (freeing its CRTC) and bring `target` up as the sole
/// output, reusing the smithay `Output`. On success the context reflects the new
/// connector/mode. On failure `ctx.pipe().drm_output` is left `None` — the caller must
/// rebuild a working output (render frames skip while it is `None`).
fn bring_up(ctx: &mut NativeRenderContext, target: &connector::Info, requested: Option<ModeInfo>) -> Result<(), String> {
    // Drop the current output FIRST so its CRTC/bandwidth is free for the target
    // (the atomic modeset of a second simultaneous pipe is rejected).
    ctx.pipe_mut().drm_output = None;
    let built = compositor_kernel_native_context_display_build::build::build(
        &ctx.drm_output_manager,
        &ctx.gpu_binding,
        &ctx.pipe().output,
        &[],
        target,
        requested,
    )?;
    let env = compositor_developer_environment_config_base::base::get();
    let new_hdr_active = env.hdr && built.hdr.hdr_capable() && ctx.vulkan_mode;
    let new_mode = Mode::from(built.drm_mode);
    ctx.pipe_mut().drm_output = Some(built.drm_output);
    ctx.pipe_mut().mode = new_mode;
    ctx.pipe_mut().current_drm_mode = built.drm_mode;
    ctx.pipe_mut().modes = built.modes;
    ctx.pipe_mut().connector = built.connector;
    ctx.pipe_mut().hdr_caps = built.hdr;
    ctx.pipe_mut().hdr_active = new_hdr_active;
    ctx.pipe_mut().hdr_signalled = false;
    ctx.pipe().output.change_current_state(Some(new_mode), None, None, None);
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
    let active = ctx.pipe().connector;
    // Current mode of every DRIVEN pipe, so each connected monitor reports its own
    // `current` in the snapshot (multi-output), not just the primary.
    let lit: Vec<(connector::Handle, ModeInfo)> = ctx
        .outputs
        .iter()
        .filter(|p| p.drm_output.is_some())
        .map(|p| (p.connector, mode_info(p.current_drm_mode)))
        .collect();
    let snap = {
        let mgr = ctx.drm_output_manager.borrow();
        let drm = mgr.device();
        let snap = compositor_kernel_native_context_display_enumerate::enumerate::enumerate(drm, active, &lit);
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
    let prev_mode = mode_info(ctx.pipe().current_drm_mode);

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
    ctx.pipe_mut().drm_output = None;
    *state.inner.kernel.get_mut(&DISPLAY_OFF_MUT) = true;
    *state.inner.kernel.get_mut(&OUTPUT_MODES_SNAPSHOT_MUT) = OutputModesSnapshot::default();
    *state.inner.kernel.get_mut(&OUTPUTS_SNAPSHOT_MUT) = OutputsSnapshot::default();
}

/// Bring a NEW connected monitor online as an ADDITIONAL output: build a fresh
/// smithay `Output` + a second pipe on a free CRTC (validating the second atomic
/// modeset over the fallback chain — `Err` on no free CRTC / bandwidth / modeset
/// failure, so it fails SOFT), place it to the right of the existing outputs, map
/// it into the `Space`, publish its `wl_output`, and push its `OutputPipe`.
fn add_output(
    state: &mut Loop,
    ctx: &mut NativeRenderContext,
    target: &connector::Info,
    requested: Option<ModeInfo>,
) -> Result<(), String> {
    // Fresh smithay Output from this connector's EDID identity.
    let output = {
        let mgr = ctx.drm_output_manager.borrow();
        let drm = mgr.device();
        let raw = compositor_kernel_drm_edid_parse_base::parse::read(drm, target);
        let parsed = raw.as_ref().and_then(compositor_kernel_drm_edid_parse_base::parse::parse);
        let identity = compositor_kernel_drm_edid_identity_base::identity::identity(
            parsed.as_ref(),
            &format!("{:?}-{}", target.interface(), target.interface_id()),
        );
        compositor_kernel_drm_output_physical_base::physical::create(target, &identity)
    };
    // Second pipe on a free CRTC (excluding the ones already lit).
    let busy: Vec<crtc::Handle> = ctx.outputs.iter().map(|p| p.crtc).collect();
    let built = compositor_kernel_native_context_display_build::build::build(
        &ctx.drm_output_manager,
        &ctx.gpu_binding,
        &output,
        &busy,
        target,
        requested,
    )?;
    // Place to the right of the existing outputs (non-overlapping horizontal tiling,
    // matching `graphic.preference.layout.output::tile_positions`).
    let x: i32 = ctx.outputs.iter().map(|p| p.mode.size.w).sum();
    let mode = Mode::from(built.drm_mode);
    compositor_kernel_drm_output_physical_base::physical::apply_initial_state(&output, mode, None, (x, 0));
    output.create_global::<compositor_support_smithay_dispatch_state_base::state::Dispatch>(
        &ctx.display_handle,
    );
    state.inner.space_state_mut().state.map_output(&output, (x, 0));
    let damage_tracker = smithay::backend::renderer::damage::OutputDamageTracker::from_output(&output);
    let env = compositor_developer_environment_config_base::base::get();
    let hdr_active = env.hdr && built.hdr.hdr_capable() && ctx.vulkan_mode;
    info!(
        "add_output: connector={:?} crtc={:?} mode={}x{} pos=({}, 0) → {} outputs total",
        built.connector,
        built.crtc,
        built.drm_mode.size().0,
        built.drm_mode.size().1,
        x,
        ctx.outputs.len() + 1,
    );
    ctx.outputs.push(compositor_kernel_native_context_render_base::render::OutputPipe {
        crtc: built.crtc,
        mode,
        output,
        damage_tracker,
        drm_output: Some(built.drm_output),
        hdr_caps: built.hdr,
        hdr_active,
        hdr_signalled: false,
        connector: built.connector,
        current_drm_mode: built.drm_mode,
        modes: built.modes,
        mode_revert: None,
        in_flight: false,
    });
    Ok(())
}

/// Dark the PRIMARY pipe (`outputs[0]`) — the always-present anchor kept even when
/// no monitor is connected (the `outputs` non-empty invariant). Mirrors `go_dark`
/// but leaves any secondary pipes to the caller's prune step.
fn go_dark_primary(state: &mut Loop, ctx: &mut NativeRenderContext) {
    ctx.outputs[0].drm_output = None;
    *state.inner.kernel.get_mut(&DISPLAY_OFF_MUT) = true;
}

/// Hotplug reconciliation (SET reconciler): converge the driven outputs to the full
/// set of connected monitors — add newly-connected ones as additional outputs,
/// drop the ones that vanished, and keep the primary (`outputs[0]`) as the anchor
/// (failing over / going dark on it). Used both at startup (light up every monitor)
/// and on every udev hotplug. Not user-confirmed — no revert gate.
pub fn reconcile(state: &mut Loop, ctx_rc: &Ctx) -> Option<OutputChange> {
    let mut ctx = ctx_rc.borrow_mut();
    // A topology change supersedes any pending user provisional switch.
    if let Some(b) = ctx.output_revert.take() {
        state.loop_handle.remove(b.timer);
    }
    let was_dark = ctx.outputs.iter().all(|p| p.drm_output.is_none());
    let connected = {
        let mgr = ctx.drm_output_manager.borrow();
        let drm = mgr.device();
        let res = compositor_kernel_drm_connector_scan_base::scan::resources(drm);
        let infos = compositor_kernel_drm_connector_scan_base::scan::connectors(drm, &res);
        infos.into_iter().filter(|i| i.state() == connector::State::Connected).collect::<Vec<_>>()
    };
    let connected_handles: Vec<connector::Handle> = connected.iter().map(|c| c.handle()).collect();

    // 1. Prune SECONDARY outputs whose connector vanished (keep `outputs[0]` anchor).
    let mut i = 1;
    while i < ctx.outputs.len() {
        if !connected_handles.contains(&ctx.outputs[i].connector) {
            let removed = ctx.outputs.remove(i);
            state.inner.space_state_mut().state.unmap_output(&removed.output);
            info!("reconcile: removed disconnected output {:?}", removed.connector);
            // `removed.drm_output` drops here → frees its CRTC.
        } else {
            i += 1;
        }
    }

    // 2. PRIMARY (`outputs[0]`): if it isn't driving a connected monitor, fail over to
    //    the preferred connected one, else go dark. (Unchanged single-output policy.)
    let primary_live = ctx.outputs[0].drm_output.is_some()
        && connected_handles.contains(&ctx.outputs[0].connector);
    if !primary_live {
        let target = {
            let mgr = ctx.drm_output_manager.borrow();
            pick_target(mgr.device(), &connected)
        };
        match target {
            Some(t) => {
                let requested = {
                    let mgr = ctx.drm_output_manager.borrow();
                    pref_mode(mgr.device(), &t)
                };
                if let Err(e) = bring_up(&mut ctx, &t, requested) {
                    warn!("reconcile primary bring-up failed: {e}; going dark");
                    go_dark_primary(state, &mut ctx);
                } else {
                    *state.inner.kernel.get_mut(&DISPLAY_OFF_MUT) = false;
                }
            }
            None => {
                go_dark_primary(state, &mut ctx);
                warn!("no monitor connected — primary dark, awaiting hotplug");
            }
        }
    }

    // 3. ADD every connected monitor not yet driven by any pipe, as an additional
    //    output. Collect first (releases the manager borrow before `add_output`).
    let driven: Vec<connector::Handle> = ctx
        .outputs
        .iter()
        .filter(|p| p.drm_output.is_some())
        .map(|p| p.connector)
        .collect();
    let to_add: Vec<connector::Info> =
        connected.iter().filter(|c| !driven.contains(&c.handle())).cloned().collect();
    for c in &to_add {
        let requested = {
            let mgr = ctx.drm_output_manager.borrow();
            pref_mode(mgr.device(), c)
        };
        match add_output(state, &mut ctx, c, requested) {
            Ok(()) => info!("reconcile: added output {:?}", c.handle()),
            Err(e) => warn!("reconcile: add_output failed for {:?}: {e}", c.handle()),
        }
    }

    // 4. Snapshots + result.
    let any_live = ctx.outputs.iter().any(|p| p.drm_output.is_some());
    if any_live {
        *state.inner.kernel.get_mut(&DISPLAY_OFF_MUT) = false;
    }
    write_snapshots(state, &ctx);
    drop(ctx);
    state.schedule_redraw();
    if !any_live {
        Some(OutputChange::WentDark)
    } else if was_dark {
        Some(OutputChange::Recovered)
    } else {
        Some(OutputChange::Changed)
    }
}
