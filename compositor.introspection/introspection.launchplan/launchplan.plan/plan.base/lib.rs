//! # compositor_introspection_launchplan_plan_base
//!
//! Façade: persistent launch plan — application data + per-handler
//! preferences + executable synthesis. Implementation lives in the
//! sibling `plan.*` crates; this crate re-exports the full public tree.

pub mod plan {
    pub use compositor_introspection_launchplan_plan_core::plan::*;
}
pub mod preferences {
    pub use compositor_introspection_launchplan_plan_preferences::preferences::*;
}
pub mod synthesizer {
    pub use compositor_introspection_launchplan_plan_core::synthesizer::*;
}
pub mod synthesizers {
    pub use compositor_introspection_launchplan_plan_synthesizers::synthesizers::*;
}
pub mod exec {
    // Unit-name helpers (sanitise_unit_name / short_random) only; the old
    // systemd-run launch machinery moved to the introspection.execution subsystem.
    pub use compositor_introspection_launchplan_plan_exec_unit::exec::*;
}

pub use plan::LaunchPlan;
pub use preferences::{PreferenceField, Preferences};
pub use synthesizer::{LaunchSynthesizer, SynthesizerRegistry};
pub use synthesizers::default_synthesizers;

pub use compositor_introspection_extraction_window_base::HandlerId;
pub use compositor_introspection_inference_hint_base::ApplicationData;
