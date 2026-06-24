//! SIGCHLD reaping. With the reaped backends the compositor remains the parent
//! of launched apps, so it must `waitpid` them or they linger as zombies and
//! recycle PIDs — which would corrupt PID-based restoration matching. Called
//! from the calloop SIGCHLD source; SIGCHLD must be blocked process-wide before
//! any thread is spawned so the signalfd is the sole consumer.

pub mod reap;
