//! The launch toggles. All compile-time `const` so dead branches drop out and
//! the `Direct` + `Inline` default lowers to exactly the historical path.

/// Which mechanism manages the lifecycle of a launched process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchBackend {
    /// Byte-identical to the historical path: `Command::spawn`, the child is
    /// detached, nothing reaps it. Zombies accumulate and PIDs recycle — kept
    /// only as the bisection baseline.
    Direct,
    /// Self-spawn (so the PID is `Child::id()`, always reliable) plus a SIGCHLD
    /// reaper wired into calloop.
    DirectReaped,
    /// Self-spawn, then adopt the live PID into a transient systemd `.scope`
    /// over the session bus. Still reaped — a scope keeps us as the parent and
    /// does NOT delegate reaping to systemd.
    SystemdScope,
}

/// Where the spawn runs relative to the calloop thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchDispatch {
    /// Spawn synchronously on the calloop thread (historical).
    Inline,
    /// Spawn on a worker thread; the outcome is posted back into the calloop
    /// loop for the general `Executed` dispatch.
    OffThread,
}

/// Active process backend. `Direct` reproduces today's behaviour exactly.
pub const LAUNCH_BACKEND: LaunchBackend = LaunchBackend::SystemdScope;

/// Active dispatch location.
pub const LAUNCH_DISPATCH: LaunchDispatch = LaunchDispatch::OffThread;

/// When `true`, a launch is expected to yield a PID and restoration records it
/// (the PID is a fallback behind the XDG activation token). When `false`, the
/// PID is discarded and restoration is token-only — used to exercise the token
/// correlation path in isolation.
pub const REQUIRE_PID: bool = true;
