//! Request / outcome records.

use uuid::Uuid;

/// A fully-resolved launch, ready to spawn. Built on the calloop thread and
/// safe to `Send` to the launch worker.
#[derive(Clone, Debug)]
pub struct LaunchRequest {
    /// Program path followed by its arguments (argv[0] is the program).
    pub argv: Vec<String>,
    /// Environment overlay applied on top of the inherited environment.
    pub env: Vec<(String, String)>,
    /// Working directory, if the plan specified one.
    pub working_dir: Option<String>,
    /// XDG activation token — the primary restoration correlation key.
    pub token: String,
    /// Unit/scope name used when the backend is `SystemdScope`.
    pub unit: String,
    /// Ties the outcome back to an originator (e.g. a placeholder uuid).
    /// `None` for launches nobody needs to correlate (plain launcher tiles).
    pub correlation: Option<Uuid>,
}

/// The result of attempting a launch. Doubles as the payload of the general
/// `Executed` bus event.
#[derive(Clone, Debug)]
pub struct LaunchOutcome {
    pub correlation: Option<Uuid>,
    pub token: String,
    /// `Some` on success — we always self-spawn, so the PID is `Child::id()`.
    /// `None` only when the spawn itself failed.
    pub pid: Option<u32>,
    /// `Err` carries a human-readable reason.
    pub result: Result<(), String>,
}
