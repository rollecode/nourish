//! Transient-capture helpers over a `LaunchPlan`: read/write the per-attribute
//! capture flag and resolve the values a placeholder matches a new window on.
//! Kept out of `plan.core` to stay within the per-crate size policy.
pub mod capture;
