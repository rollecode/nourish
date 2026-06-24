//! Non-blocking drain of exited children.

/// Reap every child that has exited since the last call, returning their pids
/// (for logging / restoration cleanup). Never blocks: `WNOHANG` makes
/// `waitpid` return 0 once no more children have exited.
///
/// We intentionally `waitpid(-1, …)` for ANY child rather than tracking a set,
/// so re-parented grandchildren are collected too. Because this owns reaping,
/// the spawn path must never call `Child::wait` itself (it would race to ECHILD).
pub fn reap_zombies() -> Vec<u32> {
    let mut reaped = Vec::new();
    let mut status: libc::c_int = 0;
    loop {
        // SAFETY: waitpid with a local status out-pointer; no other invariants.
        let pid = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };
        if pid <= 0 {
            // 0 = children exist but none exited; -1 = no children / error.
            break;
        }
        reaped.push(pid as u32);
    }
    reaped
}
