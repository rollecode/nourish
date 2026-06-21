//! [`PlaceholderUi`] — the iced UI instance.

use std::any::TypeId;
use std::sync::Arc;

use iced_core::{Element, Theme};
use iced_widget::combo_box;
use compositor_introspection_extraction_window_base::{HandlerId, HandlerRegistry};
use compositor_introspection_inference_hint_base::AttributeDescriptor;
use compositor_introspection_launchplan_plan_base::LaunchPlan;
use compositor_support_iced_core_engine_base::{IcedUi, Renderer};

use crate::message::{EnvField, PlaceholderMessage};
use crate::mode::Mode;
use crate::view;

/// The placeholder iced UI.
///
/// Holds:
/// - `canonical`: the last [`LaunchPlan`] received from the compositor.
/// - `working`: the live editing copy used while in Settings mode.
/// - `mode`: which view to render.
/// - `registry`: shared handler registry from the compositor.
/// - `combo_active`: which attribute's combo_box is currently expanded
///   (at most one). `None` means all alternatives pickers render in a
///   collapsed state.
/// - `combo_state`: iced combo_box state for the currently-active combo.
///   Rebuilt when the active attribute changes.
pub struct PlaceholderUi {
    pub(crate) canonical: LaunchPlan,
    pub(crate) session: Option<LaunchPlan>,
    pub(crate) working: LaunchPlan,
    pub(crate) mode: Mode,
    pub(crate) registry: Arc<HandlerRegistry>,
    pub(crate) combo_active: Option<&'static str>,
    pub(crate) combo_state: combo_box::State<String>,
}

impl PlaceholderUi {
    pub fn new(plan: LaunchPlan, plan_session: Option<LaunchPlan>, registry: Arc<HandlerRegistry>) -> Self {
        Self {
            working: plan.clone(),
            canonical: plan,
            session: plan_session,
            mode: Mode::View,
            registry,
            combo_active: None,
            combo_state: combo_box::State::new(Vec::new()),
        }
    }

    /// The currently-edited plan in Settings mode, or the canonical
    /// plan in View mode. Handy for rendering: both modes read the
    /// effective values via this accessor.
    pub fn shown_plan(&self) -> &LaunchPlan {
        match self.mode {
            Mode::View => &self.canonical,
            Mode::Settings => &self.working,
        }
    }
}

impl IcedUi for PlaceholderUi {
    type Message = PlaceholderMessage;

    fn update(&mut self, message: Self::Message) {
        match message {
            PlaceholderMessage::LaunchClicked => {
                // Compositor-handled. Do nothing in the UI.
            }
            PlaceholderMessage::DismissClicked => {
                // Compositor-handled. Do nothing in the UI.
            }
            
            PlaceholderMessage::RestoreClicked { } => {
                let restore_plan = self.session.clone();
                let restore_plan = if let Some(restore_plan) = restore_plan{
                    restore_plan.clone()
                } else {
                    self.canonical.clone()
                };
                self.working = restore_plan;
                
            }
            
            PlaceholderMessage::SaveClicked { .. } => {
                // Compositor-handled. UI doesn't synthesize this itself —
                // it's emitted only from the Save button handler, which
                // is in the settings view. After Save the compositor will
                // push an UpdatePlan and EnterViewMode to bring us back.
            }

            PlaceholderMessage::UpdatePlan(plan) => {
                let plan = *plan;
                // Don't trample the user's working copy if they're
                // currently editing — they may have unsaved changes.
                if self.mode == Mode::View {
                    self.working = plan.clone();
                }
                self.canonical = plan;
                self.mode = Mode::View;
            }
            PlaceholderMessage::EnterViewMode => {
                self.mode = Mode::View;
                self.working = self.canonical.clone();
            }

            PlaceholderMessage::EnterSettings => {
                self.working = self.canonical.clone();
                self.mode = Mode::Settings;
            }
            PlaceholderMessage::CancelSettings => {
                self.working = self.canonical.clone();
                self.mode = Mode::View;
            }
            PlaceholderMessage::ActiveHandlerChanged(handler) => {
                self.working.set_active_handler(handler);
            }
            PlaceholderMessage::AttributeEnabledChanged {
                descriptor_key,
                enabled,
            } => {
                if let Some(d) = self.descriptor_by_key(descriptor_key) {
                    self.working.set_enabled_raw(&d, enabled);
                }
            }
            PlaceholderMessage::AttributeCaptureToggled {
                descriptor_key,
                capture,
            } => {
                if let Some(d) = self.descriptor_by_key(descriptor_key) {
                    compositor_introspection_launchplan_plan_capture::capture::set_capture_raw(&mut self.working, &d, capture);
                }
            }
            PlaceholderMessage::AttributeTextChanged {
                descriptor_key,
                value,
            } => {
                self.apply_text_change(descriptor_key, value);
            }
            PlaceholderMessage::AttributeBoolChanged {
                descriptor_key,
                value,
            } => {
                self.apply_bool_change(descriptor_key, value);
            }
            PlaceholderMessage::AttributeStringListItemChanged {
                descriptor_key,
                index,
                value,
            } => {
                self.mutate_string_list(descriptor_key, |list| {
                    if let Some(slot) = list.get_mut(index) {
                        *slot = value;
                    }
                });
            }
            PlaceholderMessage::AttributeStringListAdd { descriptor_key } => {
                self.mutate_string_list(descriptor_key, |list| {
                    list.push(String::new());
                });
            }
            PlaceholderMessage::AttributeStringListRemove {
                descriptor_key,
                index,
            } => {
                self.mutate_string_list(descriptor_key, |list| {
                    if index < list.len() {
                        list.remove(index);
                    }
                });
            }
            PlaceholderMessage::AttributeEnvPairChanged {
                descriptor_key,
                index,
                field,
                value,
            } => {
                self.mutate_env_pair_list(descriptor_key, |list| {
                    if let Some(pair) = list.get_mut(index) {
                        match field {
                            EnvField::Key => pair.key = value,
                            EnvField::Value => pair.value = value,
                        }
                    }
                });
            }
            PlaceholderMessage::AttributeEnvPairAdd { descriptor_key } => {
                self.mutate_env_pair_list(descriptor_key, |list| {
                    list.push(compositor_introspection_extraction_window_base::EnvPair {
                        key: String::new(),
                        value: String::new(),
                    });
                });
            }
            PlaceholderMessage::AttributeEnvPairRemove {
                descriptor_key,
                index,
            } => {
                self.mutate_env_pair_list(descriptor_key, |list| {
                    if index < list.len() {
                        list.remove(index);
                    }
                });
            }

            PlaceholderMessage::ComboOpen { descriptor_key } => {
                self.open_combo(descriptor_key);
            }
            PlaceholderMessage::ComboClose => {
                self.combo_active = None;
                self.combo_state = combo_box::State::new(Vec::new());
            }
            PlaceholderMessage::AlternativeSelected {
                descriptor_key,
                label,
            } => {
                self.apply_alternative(descriptor_key, &label);
                self.combo_active = None;
                self.combo_state = combo_box::State::new(Vec::new());
            }
        }
    }

    fn view(&self) -> Element<'_, Self::Message, Theme, Renderer> {
        view::root_view(self)
    }
}

// ── Internal helpers ─────────────────────────────────────────────────

impl PlaceholderUi {
    /// Find a descriptor by its key string among all descriptors that
    /// apply to the working plan.
    pub(crate) fn descriptor_by_key(&self, key: &str) -> Option<AttributeDescriptor> {
        self.all_descriptors().into_iter().find(|d| d.key == key)
    }

    /// All descriptors visible in the settings UI for the working plan.
    /// Identity + Launch + active handler's scoped attributes.
    pub(crate) fn all_descriptors(&self) -> Vec<AttributeDescriptor> {
        compositor_introspection_inference_hint_base::all_descriptors_for(
            &self.registry,
            self.working.active_handler,
        )
    }

    fn apply_text_change(&mut self, key: &str, value: String) {
        let Some(d) = self.descriptor_by_key(key) else { return };
        use compositor_introspection_extraction_window_base::AttributeKind as K;
        match &d.kind {
            K::Text | K::EnumOf(_) => {
                self.working.set_pref_raw(
                    &d,
                    Arc::new(value),
                    TypeId::of::<String>(),
                );
            }
            K::Path => {
                use std::path::PathBuf;
                self.working.set_pref_raw(
                    &d,
                    Arc::new(PathBuf::from(value)),
                    TypeId::of::<PathBuf>(),
                );
            }
            K::Custom(tag) if *tag == "chrome_profile" => {
                self.working.set_pref_raw(
                    &d,
                    Arc::new(value),
                    TypeId::of::<String>(),
                );
            }
            // Other custom kinds (sandbox, handler_id, profile_list,
            // env_pair_list) don't accept Text input.
            _ => {}
        }
    }

    fn apply_bool_change(&mut self, key: &str, value: bool) {
        let Some(d) = self.descriptor_by_key(key) else { return };
        self.working
            .set_pref_raw(&d, Arc::new(value), TypeId::of::<bool>());
    }

    fn mutate_string_list<F: FnOnce(&mut Vec<String>)>(&mut self, key: &str, f: F) {
        let Some(d) = self.descriptor_by_key(key) else { return };
        let mut current: Vec<String> = self
            .working
            .current_raw(&d)
            .and_then(|a| a.downcast_ref::<Vec<String>>().cloned())
            .unwrap_or_default();
        f(&mut current);
        self.working.set_pref_raw(
            &d,
            Arc::new(current),
            TypeId::of::<Vec<String>>(),
        );
    }

    fn mutate_env_pair_list<F>(&mut self, key: &str, f: F)
    where
        F: FnOnce(&mut Vec<compositor_introspection_extraction_window_base::EnvPair>),
    {
        let Some(d) = self.descriptor_by_key(key) else { return };
        let mut current: Vec<compositor_introspection_extraction_window_base::EnvPair> = self
            .working
            .current_raw(&d)
            .and_then(|a| {
                a.downcast_ref::<Vec<compositor_introspection_extraction_window_base::EnvPair>>()
                    .cloned()
            })
            .unwrap_or_default();
        f(&mut current);
        self.working.set_pref_raw(
            &d,
            Arc::new(current),
            TypeId::of::<Vec<compositor_introspection_extraction_window_base::EnvPair>>(),
        );
    }

    /// List of handlers the user can switch to via the picker.
    /// Returns id + display name pairs in deterministic order.
    pub(crate) fn handler_choices(&self) -> Vec<(HandlerId, String)> {
        let mut ids: Vec<HandlerId> = self.registry.ids().collect();
        ids.sort_by_key(|id| id.name());
        ids.into_iter()
            .map(|id| (id, id.to_string()))
            .collect()
    }

    // ── Alternatives picker (combo_box) ──────────────────────────

    /// Display-string labels for the alternatives of one attribute, in
    /// the same order [`alternatives_for`] returns them. Used to
    /// populate the combo_box options list.
    ///
    /// Each label includes a summary of the value, its source, and
    /// confidence so the user can disambiguate.
    pub(crate) fn alternative_labels(&self, descriptor: &AttributeDescriptor) -> Vec<String> {
        self.alternatives_for(descriptor)
            .iter()
            .map(|alt| {
                format!(
                    "{}  ·  {:?}  ·  {}",
                    crate::view::settings::attribute_widget::summarize_value(&alt.value),
                    alt.source.method,
                    confidence_label(alt.confidence),
                )
            })
            .collect()
    }

    /// Get the alternatives for an attribute by descriptor.
    pub(crate) fn alternatives_for(
        &self,
        descriptor: &AttributeDescriptor,
    ) -> Vec<compositor_introspection_extraction_window_base::RawAlternative> {
        self.working
            .application_data
            .hints
            .available_raw(descriptor.key)
    }

    fn open_combo(&mut self, key: &'static str) {
        let Some(d) = self.descriptor_by_key(key) else { return };
        let labels = self.alternative_labels(&d);
        self.combo_state = combo_box::State::new(labels);
        self.combo_active = Some(key);
    }

    fn apply_alternative(&mut self, key: &str, chosen_label: &str) {
        let Some(d) = self.descriptor_by_key(key) else { return };
        let alternatives = self.alternatives_for(&d);
        let labels = self.alternative_labels(&d);
        let Some(index) = labels.iter().position(|l| l == chosen_label) else { return };
        let alt = &alternatives[index];
        let type_id = (*alt.value).type_id();
        self.working.set_pref_raw(&d, alt.value.clone(), type_id);
    }
}

fn confidence_label(c: compositor_introspection_extraction_window_base::Confidence) -> &'static str {
    use compositor_introspection_extraction_window_base::Confidence;
    match c {
        Confidence::High => "high",
        Confidence::Medium => "medium",
        Confidence::Low => "low",
    }
}
