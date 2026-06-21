//! Identity attributes carried by the live surface (not derived from the
//! process tree): the window title and the Wayland app id / X11 WM_CLASS.
//! Both are `AttributeCategory::Identity` so they edit and persist exactly
//! like the attributes in the sibling `attributes.identity` crate; they
//! exist primarily so a placeholder can capture a new window by matching
//! its title and/or app id.

use compositor_introspection_extraction_window_hints_attribute::attribute::HintAttribute;
use compositor_introspection_extraction_window_hints_descriptor::descriptor::{AttributeDescriptor, AttributeKind};
use compositor_introspection_extraction_window_hints_id::category::AttributeCategory;

/// Live window title (xdg_toplevel title / X11 `WM_NAME`).
#[derive(Debug)]
pub struct Title;
impl HintAttribute for Title {
    type Value = String;
    fn name() -> &'static str { "title" }
    fn category() -> AttributeCategory { AttributeCategory::Identity }
    fn descriptor() -> AttributeDescriptor { AttributeDescriptor::new(Self::name(), "Window title", Self::category(), AttributeKind::Text) }
}

/// Wayland `app_id` (xdg-shell) or X11 `WM_CLASS` surfaced via xwayland.
#[derive(Debug)]
pub struct AppId;
impl HintAttribute for AppId {
    type Value = String;
    fn name() -> &'static str { "app_id" }
    fn category() -> AttributeCategory { AttributeCategory::Identity }
    fn descriptor() -> AttributeDescriptor { AttributeDescriptor::new(Self::name(), "App ID", Self::category(), AttributeKind::Text) }
}
