use smithay::reexports::calloop::channel::Sender as CalloopSender;

use compositor_support_system_storage_token_base::base::{Token, TokenMut};
use compositor_introspection_execution_launch_policy::policy::{LaunchDispatch, LAUNCH_DISPATCH};
use compositor_introspection_execution_launch_execute::execute::execute;
use compositor_introspection_execution_launch_dispatch::dispatch::LaunchWorker;
use compositor_introspection_execution_launch_types::types::{LaunchOutcome, LaunchRequest};

/// App-launch driver. Held in kernel storage; populated by the loader's
/// `install`. Every outcome is posted on `outcome_tx` so orchestration can
/// broadcast it; the executor itself stays unaware of placeholders / the bus.
/// `scope` (adopt into a systemd transient scope) is decided once at install
/// from a runtime systemd probe, so the same value covers every launch.
pub struct Executor {
    worker: Option<LaunchWorker>,
    outcome_tx: CalloopSender<LaunchOutcome>,
    base_env: Vec<(String, String)>,
    scope: bool,
}

impl Executor {
    pub fn new(
        worker: Option<LaunchWorker>,
        outcome_tx: CalloopSender<LaunchOutcome>,
        base_env: Vec<(String, String)>,
        scope: bool,
    ) -> Self {
        Self { worker, outcome_tx, base_env, scope }
    }

    /// Launch `request`. The faithful base env is prepended; caller-supplied
    /// entries (e.g. the activation token) override it. `Inline` returns the
    /// outcome immediately (and posts it for broadcast); `OffThread` returns
    /// `None` and the outcome is posted when the worker finishes.
    pub fn launch(&self, mut request: LaunchRequest) -> Option<LaunchOutcome> {
        let mut env = self.base_env.clone();
        env.append(&mut request.env);
        request.env = env;

        if matches!(LAUNCH_DISPATCH, LaunchDispatch::OffThread) {
            if let Some(worker) = &self.worker {
                worker.submit(request);
                return None;
            }
        }
        let outcome = execute(&request, self.scope);
        let _ = self.outcome_tx.send(outcome.clone());
        Some(outcome)
    }
}

/// Kernel storage slot for the executor (driver data), set by the loader.
pub static EXECUTOR: Token<Option<Executor>> = Token::new();
pub static EXECUTOR_MUT: TokenMut<Option<Executor>> = TokenMut::new(&EXECUTOR);
