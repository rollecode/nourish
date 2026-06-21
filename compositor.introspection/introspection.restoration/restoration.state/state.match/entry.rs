//! [`match_window`]: the pure matching entry point.

use uuid::Uuid;
use compositor_introspection_extraction_window_base::{InferredHints, MetaNode};
use compositor_introspection_extraction_window_hints_codec::codec;
use compositor_introspection_extraction_window_hints_codec_register::register;
use compositor_introspection_launchplan_plan_capture::capture as plan_capture;
use compositor_introspection_restoration_state_matcher::matcher::MatchResult;
use compositor_introspection_restoration_state_pending::pending::PendingRestoration;
use compositor_introspection_restoration_state_registry::registry::MatcherRegistry;

/// Decide which pending restoration (if any) a newly-appeared window
/// satisfies. Pure function.
///
/// Iterates `pendings` in order (FIFO). For each, dispatches to the
/// matcher registered for `pending.plan.active_handler`, falling back
/// to the registry's generic matcher if none is registered. First
/// [`MatchResult::Yes`] wins; the matched pending's id is returned.
///
/// Inputs:
/// - `pendings`: in-flight launches the compositor is waiting on.
/// - `candidate`: the new window's captured metadata.
/// - `candidate_hints`: hints inferred for the new window (matchers may
///   use them for handler-specific signals like terminal kind).
/// - `candidate_token`: the activation token the surface received via
///   Wayland's xdg-activation protocol (set by the compositor from
///   `request_activation`). `None` if the surface didn't carry one.
/// - `matchers`: the per-handler matcher registry.
pub fn match_window(
    pendings: &[PendingRestoration],
    candidate: &MetaNode,
    candidate_hints: &InferredHints,
    candidate_token: Option<&str>,
    matchers: &MatcherRegistry,
) -> Option<Uuid> {
    for pending in pendings {
        let matcher = pending
            .plan
            .active_handler
            .and_then(|id| matchers.get(id))
            .or_else(|| matchers.fallback());

        // Explicit-launch signals (activation token / PID tree) take
        // precedence: if the per-handler matcher claims the window, bind it.
        if let Some(matcher) = matcher {
            if matcher.matches(pending, candidate, candidate_hints, candidate_token) == MatchResult::Yes {
                return Some(pending.id);
            }
        }

        // Otherwise fall through to transient capture: a placeholder with
        // capture-armed attributes adopts a window whose values match, even
        // though no Launch spawned it.
        if capture_matches(pending, candidate_hints) {
            return Some(pending.id);
        }
    }
    None
}

/// Transient-capture predicate: every attribute the placeholder armed for
/// capture must exactly equal the new window's value. Empty capture set =>
/// never matches (the placeholder only restores via an explicit Launch).
///
/// Values are type-erased (`Arc<dyn Any>`), so we compare their codec-encoded
/// JSON — the same encoding persistence uses — which gives value equality
/// without per-type downcasts. A missing value on either side, or an
/// unregistered codec, fails the match (exact equality requires both present).
fn capture_matches(pending: &PendingRestoration, candidate_hints: &InferredHints) -> bool {
    let keys = plan_capture::capture_keys(&pending.plan);
    if keys.is_empty() {
        return false;
    }
    register::register_standard_codecs();
    for key in keys {
        let (Some(stored), Some(live)) =
            (plan_capture::current_raw_by_key(&pending.plan, key), candidate_hints.best_raw(key))
        else {
            return false;
        };
        let stored_json = codec::encode(key, &stored);
        if stored_json.is_none() || stored_json != codec::encode(key, &live) {
            return false;
        }
    }
    true
}
