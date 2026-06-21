use compositor_introspection_extraction_window_base::{Confidence, HintSource, Meta, MetaNode};
use compositor_introspection_extraction_window_hints_codec::codec;
use compositor_introspection_extraction_window_hints_codec_register::register;
use compositor_introspection_inference_hint_base::{ApplicationData, InferredHints};
use compositor_introspection_launchplan_plan_base::LaunchPlan;
use compositor_introspection_launchplan_plan_preferences::Preferences;

/// One persisted inferred hint: its codec-encoded value + provenance. Restored
/// verbatim into `InferredHints`, so the full hint set survives a reboot.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PersistedHint {
    pub name: String,
    pub value: serde_json::Value,
    pub source: HintSource,
    pub confidence: Confidence,
}

/// One persisted preference override (a user edit) for an attribute.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PersistedPrefField {
    pub name: String,
    pub enabled: bool,
    /// Whether this attribute is armed for transient capture. Defaults to
    /// `false` so configs written before the capture feature load cleanly.
    #[serde(default)]
    pub capture: bool,
    pub value: Option<serde_json::Value>,
}

/// A handler's preference set, keyed by its stable handler name.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PersistedHandlerPrefs {
    pub handler: String,
    pub fields: Vec<PersistedPrefField>,
}

/// A placeholder's full launch state: every inferred hint AND the user's edits,
/// reconstructable into an editable `LaunchPlan` without the process tree / pid.
#[derive(Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PersistedLaunch {
    pub hints: Vec<PersistedHint>,
    pub global_prefs: Vec<PersistedPrefField>,
    pub handler_prefs: Vec<PersistedHandlerPrefs>,
    pub active_handler: Option<String>,
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlaceholderRecord {
    pub position: (i32, i32),
    pub size: (i32, i32),
    pub persistent: bool,
    pub launch: PersistedLaunch,
}

/// Project a live launch plan to its full persisted state via the codec registry.
pub fn to_persisted(plan: &LaunchPlan) -> PersistedLaunch {
    register::register_standard_codecs();
    let hints = plan
        .application_data
        .hints
        .items
        .iter()
        .filter_map(|it| {
            codec::encode(it.attr_name, &it.value).map(|value| PersistedHint {
                name: it.attr_name.to_string(),
                value,
                source: it.source.clone(),
                confidence: it.confidence,
            })
        })
        .collect();
    let handler_prefs = plan
        .handler_preferences
        .iter()
        .map(|(id, prefs)| PersistedHandlerPrefs {
            handler: id.name().to_string(),
            fields: persist_prefs(prefs),
        })
        .collect();
    PersistedLaunch {
        hints,
        global_prefs: persist_prefs(&plan.global_preferences),
        handler_prefs,
        active_handler: plan.active_handler.map(|h| h.name().to_string()),
    }
}

fn persist_prefs(prefs: &Preferences) -> Vec<PersistedPrefField> {
    prefs
        .iter()
        .map(|(name, f)| PersistedPrefField {
            name: name.to_string(),
            enabled: f.enabled,
            capture: f.capture,
            value: f.override_value.as_ref().and_then(|arc| codec::encode(name, arc)),
        })
        .collect()
}

/// Rebuild an editable launch plan from saved state: hints re-pushed verbatim
/// (so `LaunchPlan::new` re-derives the handler), then the user's edits re-applied.
pub fn to_launch_plan(l: &PersistedLaunch) -> LaunchPlan {
    register::register_standard_codecs();
    let mut hints = InferredHints::new();
    for ph in &l.hints {
        if let Some(item) = codec::make_hint_item(&ph.name, &ph.value, ph.source.clone(), ph.confidence) {
            hints.items.push(item);
        }
    }
    let mut plan = LaunchPlan::new(ApplicationData::new(MetaNode::leaf(Meta::default()), hints));
    apply_prefs(&mut plan.global_preferences, &l.global_prefs);
    for hp in &l.handler_prefs {
        if let Some(id) = register::handler_id_from_name(&hp.handler) {
            apply_prefs(plan.handler_prefs_mut(id), &hp.fields);
        }
    }
    if let Some(name) = &l.active_handler {
        plan.set_active_handler(register::handler_id_from_name(name));
    }
    plan
}

fn apply_prefs(prefs: &mut Preferences, fields: &[PersistedPrefField]) {
    for f in fields {
        let Some(name) = codec::static_name(&f.name) else { continue };
        prefs.set_enabled_by_name(name, f.enabled);
        prefs.set_capture_by_name(name, f.capture);
        if let Some(val) = &f.value {
            if let (Some(arc), Some(tid)) = (codec::decode(&f.name, val), codec::value_type_id(&f.name)) {
                prefs.set_raw(name, arc, tid);
            }
        }
    }
}
