//! Free functions (not `LaunchPlan` methods, to respect crate size limits)
//! for the per-attribute transient-capture flag and match-value resolution.

use std::any::Any;
use std::sync::Arc;

use compositor_introspection_extraction_window_base::AttributeDescriptor;
use compositor_introspection_launchplan_plan_base::LaunchPlan;
use compositor_introspection_launchplan_plan_query::query;

/// Whether the descriptor's attribute is armed for capture in this plan.
pub fn is_pref_capture(plan: &LaunchPlan, descriptor: &AttributeDescriptor) -> bool {
    match query::prefs_for(&plan.global_preferences, &plan.handler_preferences, plan.active_handler, &descriptor.category) {
        Some(p) => p.is_capture_by_name(descriptor.key),
        None => false,
    }
}

/// Arm or disarm capture for the descriptor's attribute.
pub fn set_capture_raw(plan: &mut LaunchPlan, descriptor: &AttributeDescriptor, capture: bool) {
    query::prefs_for_mut(&mut plan.global_preferences, &mut plan.handler_preferences, &descriptor.category)
        .set_capture_by_name(descriptor.key, capture);
}

/// Attribute keys the user armed for capture, across the global prefs and the
/// active handler's prefs. Empty => this placeholder only restores via an
/// explicit Launch (it never adopts a window on its own).
pub fn capture_keys(plan: &LaunchPlan) -> Vec<&'static str> {
    let mut keys: Vec<&'static str> = plan
        .global_preferences
        .iter()
        .filter(|(_, f)| f.capture)
        .map(|(k, _)| k)
        .collect();
    if let Some(prefs) = plan.active_handler_prefs() {
        keys.extend(prefs.iter().filter(|(_, f)| f.capture).map(|(k, _)| k));
    }
    keys
}

/// Effective value for capture matching, keyed by attribute name: the enabled
/// user override if any, else the inferred best value. `get_raw` already
/// returns `None` for a disabled override, so a disabled attribute falls
/// through to its inferred value.
pub fn current_raw_by_key(plan: &LaunchPlan, key: &str) -> Option<Arc<dyn Any + Send + Sync>> {
    plan.global_preferences
        .get_raw(key)
        .or_else(|| plan.active_handler_prefs().and_then(|p| p.get_raw(key)))
        .or_else(|| plan.application_data.hints.best_raw(key))
}
