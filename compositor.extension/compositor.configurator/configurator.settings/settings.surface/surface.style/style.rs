//! Tactical-HUD palette + container styles for the full-width settings surface:
//! a dark navy field with cyan accents and thin hairline-bordered cards/strips.
//! Button/slider/toggler styles live in `surface.control`.
use iced_core::{Background, Border, Color, Theme};
use iced_widget::container::Style as Container;

pub const ACCENT: Color = Color { r: 0.27, g: 0.78, b: 0.88, a: 1.0 };
pub const TEXT: Color = Color { r: 0.80, g: 0.86, b: 0.90, a: 1.0 };
pub const MUTED: Color = Color { r: 0.42, g: 0.50, b: 0.57, a: 1.0 };
pub const LINE: Color = Color { r: 0.27, g: 0.78, b: 0.88, a: 0.16 };

pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
    Color { r, g, b, a }
}
fn bg(c: Color) -> Option<Background> {
    Some(Background::Color(c))
}
fn lined(fill: Color, border: Color, width: f32, radius: f32) -> Container {
    Container {
        background: bg(fill),
        text_color: Some(TEXT),
        border: Border { color: border, width, radius: radius.into() },
        ..Default::default()
    }
}

/// Full-screen field (slightly translucent so the frozen blur reads through).
pub fn backdrop(_t: &Theme) -> Container {
    lined(rgba(0.027, 0.043, 0.059, 0.96), Color::TRANSPARENT, 0.0, 0.0)
}
/// A bordered content card / info cell.
pub fn card(_t: &Theme) -> Container {
    lined(rgba(0.063, 0.086, 0.110, 0.55), LINE, 1.0, 4.0)
}
/// Left module-sidebar rail.
pub fn sidebar(_t: &Theme) -> Container {
    lined(rgba(0.016, 0.027, 0.039, 0.55), LINE, 1.0, 0.0)
}
/// Top title bar / bottom status strip (hairline rule).
pub fn strip(_t: &Theme) -> Container {
    lined(rgba(0.035, 0.055, 0.075, 0.5), LINE, 1.0, 0.0)
}
/// A keybinding chip (outlined, faint fill).
pub fn chip(_t: &Theme) -> Container {
    lined(rgba(0.27, 0.78, 0.88, 0.07), rgba(0.27, 0.78, 0.88, 0.30), 1.0, 3.0)
}
/// A small accent status dot (cyan disc).
pub fn dot(_t: &Theme) -> Container {
    lined(ACCENT, Color::TRANSPARENT, 0.0, 5.0)
}
/// The filled portion of a meter bar (storage/memory).
pub fn meter_fill(_t: &Theme) -> Container {
    lined(ACCENT, Color::TRANSPARENT, 0.0, 2.0)
}
/// The empty track of a meter bar.
pub fn meter_track(_t: &Theme) -> Container {
    lined(rgba(1.0, 1.0, 1.0, 0.06), LINE, 1.0, 2.0)
}
