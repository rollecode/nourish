use compositor_orchestration_core_state_base::Loop;

/// Advance the important renderer-free control-plane once. Called from the kernel's
/// dark-tick timer (only while dark) — never touches the renderer, so it is safe
/// with no output. Idempotent: dispatching/draining empty queues is a no-op, so it
/// coexists with the per-frame path that does the same work while rendering.
pub fn pump(state: &mut Loop) {
    // Deliver queued channel events to the active world's systems (the per-frame
    // `worlds.dispatch` doesn't run while dark) — e.g. OutputChange → capture stop.
    {
        let (worlds, kernel) = (&mut state.inner.worlds, &state.inner.kernel);
        worlds.active_mut().dispatch(kernel);
    }
    // RPC remote-control: drain + execute (renderer-free; mirrors `scene.frame::buffers`).
    let incoming: Vec<_> = state
        .inner
        .kernel
        .get_mut(&compositor_orchestration_driver_remote_base::base::RPC_MUT)
        .incoming_buffer
        .drain(..)
        .collect();
    for message in incoming {
        compositor_remote_client_handle_base::handle::execute(state, message);
    }
}
