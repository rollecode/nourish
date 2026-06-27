//! Live output-mode changes from the settings window with a user-confirmed fault
//! gate: a provisional apply the user must KEEP within a timeout or it
//! auto-reverts. Driven via OUTPUT_MODE_REQUEST (the rim can't reach the DRM
//! output); drained from the kernel input loop, like the lid.
use compositor_kernel_gles_element_wrap_base::wrap::GlesElementWrapper;
use compositor_kernel_native_context_render_base::render::NativeRenderContext;
use compositor_orchestration_core_state_base::state::StateDRMBinding;
use compositor_orchestration_core_state_base::Loop;
use compositor_orchestration_draw_scene_element::element::SceneElement;
use compositor_orchestration_driver_output_base::base::{ApplyResult, OutputModeRequest, OUTPUT_MODE_REQUEST_MUT, OUTPUT_MODE_RESULT_MUT};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::RegistrationToken;
use smithay::reexports::drm::control::Mode as DrmMode;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
/// How long a provisionally-applied mode survives without an explicit Keep.
const CONFIRM_TIMEOUT: Duration = Duration::from_secs(15);
type Ctx = Rc<RefCell<NativeRenderContext>>;
fn set_result(state: &mut Loop, r: ApplyResult) {
    *state.inner.kernel.get_mut(&OUTPUT_MODE_RESULT_MUT) = Some(r);
}
/// Apply `mode` to the running pipe and propagate to the smithay Output.
fn set_mode_now(ctx: &mut NativeRenderContext, mode: DrmMode) -> Result<(), String> {
    let gpu = ctx.gpu_binding.clone();
    let mut binding = gpu.borrow_mut();
    let StateDRMBinding { gpus, primary } = &mut *binding;
    let mut renderer = gpus.single_renderer(primary).map_err(|e| format!("renderer: {e:?}"))?;
    compositor_kernel_scanout_surface_reconfigure_base::reconfigure::set_output_mode::<_, GlesElementWrapper<SceneElement<GlesRenderer>>>(&mut ctx.drm_output, mode, &mut renderer)?;
    drop(binding);
    let m = smithay::output::Mode::from(mode);
    ctx.mode = m;
    ctx.current_drm_mode = mode;
    ctx.output.change_current_state(Some(m), None, None, None);
    Ok(())
}
pub fn drain(state: &mut Loop, ctx_rc: &Ctx) {
    let Some(req) = state.inner.kernel.get_mut(&OUTPUT_MODE_REQUEST_MUT).take() else { return };
    match req {
        OutputModeRequest::Apply { width, height, refresh_mhz } => apply(state, ctx_rc, width, height, refresh_mhz),
        OutputModeRequest::Confirm => finish(state, ctx_rc, false),
        OutputModeRequest::Revert => finish(state, ctx_rc, true),
    }
}
fn apply(state: &mut Loop, ctx_rc: &Ctx, w: u16, h: u16, mhz: u32) {
    let mut ctx = ctx_rc.borrow_mut();
    let Some(target) = ctx.modes.iter().copied().find(|m| m.size() == (w, h) && m.vrefresh() * 1000 == mhz) else {
        drop(ctx);
        warn!("requested mode {w}x{h} not advertised");
        return set_result(state, ApplyResult::Failed);
    };
    // Keep the ORIGINAL baseline across re-applies; cancel any pending timer.
    let baseline = match ctx.mode_revert.take() {
        Some((prev, token)) => { state.loop_handle.remove(token); prev }
        None => ctx.current_drm_mode,
    };
    if let Err(e) = set_mode_now(&mut ctx, target) {
        warn!("live mode apply failed: {e}");
        let _ = set_mode_now(&mut ctx, baseline);
        drop(ctx);
        return set_result(state, ApplyResult::Failed);
    }
    drop(ctx);
    state.schedule_redraw();
    let token = arm(state, ctx_rc, baseline);
    ctx_rc.borrow_mut().mode_revert = Some((baseline, token));
    set_result(state, ApplyResult::Provisional);
}
/// `revert=false` keeps a pending mode (Confirm); `true` restores baseline.
fn finish(state: &mut Loop, ctx_rc: &Ctx, revert: bool) {
    let mut ctx = ctx_rc.borrow_mut();
    let Some((baseline, token)) = ctx.mode_revert.take() else { return };
    state.loop_handle.remove(token);
    if revert {
        if let Err(e) = set_mode_now(&mut ctx, baseline) { warn!("revert failed: {e}"); }
        drop(ctx);
        state.schedule_redraw();
        set_result(state, ApplyResult::Reverted);
    } else {
        drop(ctx);
        set_result(state, ApplyResult::Confirmed);
    }
}
fn arm(state: &mut Loop, ctx_rc: &Ctx, baseline: DrmMode) -> RegistrationToken {
    let ctx = ctx_rc.clone();
    state.loop_handle.insert_source(Timer::from_duration(CONFIRM_TIMEOUT), move |_, _, state: &mut Loop| {
        let mut c = ctx.borrow_mut();
        if c.mode_revert.take().is_some() {
            if let Err(e) = set_mode_now(&mut c, baseline) { warn!("auto-revert failed: {e}"); }
            drop(c);
            state.schedule_redraw();
            set_result(state, ApplyResult::Reverted);
        }
        TimeoutAction::Drop
    }).expect("mode revert watchdog registration failed")
}
