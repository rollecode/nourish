//! KEYBOARD LAYOUT section of the Misc tab. Emits a whole `KeyboardLayout` per edit
//! (`SettingsMessage::Keyboard`), applied live + persisted by the handler. `Env`
//! reads the `XKB_DEFAULT_*` environment; `Manual` uses the explicit fields, which
//! are visually disabled (greyed, non-editable) while the source is `Env`.
use compositor_developer_environment_preference_base::base::{KeyboardLayout, LayoutSource};
use compositor_support_iced_core_engine_base::Renderer;
use compositor_configurator_settings_surface_message::message::SettingsMessage;
use compositor_configurator_settings_surface_style::style;
use compositor_configurator_settings_surface_control::control;
use iced_core::{Alignment, Element, Length, Theme};
use iced_widget::{button, column, container, row, text, text_input, toggler};

type El<'a> = Element<'a, SettingsMessage, Theme, Renderer>;

/// The most common xkb layouts offered in the list, `(code, label)`. Nordic
/// layouts (se/no/dk/fi/is) are grouped after the English ones.
const LAYOUTS: &[(&str, &str)] = &[
    ("us", "English (US)"),
    ("gb", "English (UK)"),
    ("se", "Swedish"),
    ("no", "Norwegian"),
    ("dk", "Danish"),
    ("fi", "Finnish"),
    ("is", "Icelandic"),
    ("de", "German"),
    ("fr", "French"),
    ("es", "Spanish"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("nl", "Dutch"),
    ("ru", "Russian"),
    ("pl", "Polish"),
];

fn card<'a>(inner: El<'a>) -> El<'a> {
    container(inner).style(style::card).width(Length::Fill).into()
}

/// Build the section's rows for splicing into the Misc tab's scrollable list.
pub fn rows<'a>(k: &'a KeyboardLayout) -> Vec<El<'a>> {
    let manual = k.source == LayoutSource::Manual;

    let head = column![
        text("KEYBOARD LAYOUT").size(14).color(style::ACCENT),
        text("Applied live. Use the environment (XKB_DEFAULT_*) or set it explicitly.").size(11).color(style::MUTED),
    ].spacing(4);

    // Source toggle: on = use the environment (disables the manual controls below).
    let src = k.clone();
    let source_row = card(
        row![
            text("Use environment (XKB_DEFAULT_*)").width(Length::Fill),
            toggler(k.source == LayoutSource::Env)
                .on_toggle(move |env| {
                    let mut x = src.clone();
                    x.source = if env { LayoutSource::Env } else { LayoutSource::Manual };
                    SettingsMessage::Keyboard(x)
                })
                .style(control::toggler),
        ].align_y(Alignment::Center).spacing(10).padding(12).into(),
    );

    // Layout: a selectable list (● = current, accent = selected). Clickable only in
    // Manual mode; under Env every row uses the greyed `disabled` style with no
    // `on_press`, so the whole list looks and behaves inert.
    let mut out: Vec<El<'a>> = vec![head.into(), source_row, text("LAYOUT").size(11).color(style::MUTED).into()];
    for (code, label) in LAYOUTS {
        let selected = k.layout == *code;
        let mark = if selected { "●" } else { "○" };
        let b = button(text(format!("{mark}  {label}  ({code})")).size(13)).width(Length::Fill);
        let b = if manual {
            let mut x = k.clone();
            x.layout = code.to_string();
            (if selected { b.style(control::accent) } else { b.style(control::action) }).on_press(SettingsMessage::Keyboard(x))
        } else {
            b.style(control::disabled)
        };
        out.push(b.into());
    }

    // Variant + options: text fields. Always styled with `control::field` (which
    // greys itself when disabled); `on_input` is attached only in Manual mode, so
    // omitting it under Env both disables editing and greys the field.
    let variant_ctl = {
        let f = text_input("variant (optional)", &k.variant).width(Length::Fixed(240.0)).style(control::field);
        if manual {
            let base = k.clone();
            f.on_input(move |s| { let mut x = base.clone(); x.variant = s; SettingsMessage::Keyboard(x) })
        } else {
            f
        }
    };
    out.push(card(
        row![text("Variant").width(Length::Fill), variant_ctl].align_y(Alignment::Center).spacing(10).padding(12).into(),
    ));

    let options_ctl = {
        let f = text_input("e.g. grp:alt_shift_toggle,caps:escape", &k.options).width(Length::Fixed(240.0)).style(control::field);
        if manual {
            let base = k.clone();
            f.on_input(move |s| { let mut x = base.clone(); x.options = s; SettingsMessage::Keyboard(x) })
        } else {
            f
        }
    };
    out.push(card(
        row![text("Options").width(Length::Fill), options_ctl].align_y(Alignment::Center).spacing(10).padding(12).into(),
    ));

    out
}
