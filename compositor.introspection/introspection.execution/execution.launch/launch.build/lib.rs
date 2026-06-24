//! Build a `LaunchRequest` from a `LaunchPlan` using only public plan APIs — no
//! launchplan edit. Mirrors the historical `execute_with_env` command building
//! so the `Direct` backend stays byte-identical.

pub mod build;
