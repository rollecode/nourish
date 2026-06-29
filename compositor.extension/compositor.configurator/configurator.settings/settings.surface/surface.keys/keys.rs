//! Keyboard-bindings column (right side of INPUT): each shortcut's label + combo.
//! Editable rows take a combo string ("Super+K"); built-ins show a static chip.
use compositor_developer_environment_keybinding_base::base::KeyRow;
use compositor_support_iced_core_engine_base::Renderer;
use compositor_configurator_settings_surface_message::message::SettingsMessage;
use compositor_configurator_settings_surface_style::style;
use compositor_configurator_settings_surface_control::control;
use iced_core::alignment::Horizontal;
use iced_core::{Alignment, Element, Length, Padding, Theme};
use iced_widget::{button, column, container, row, scrollable, text, text_input, toggler, Column};

type El<'a> = Element<'a, SettingsMessage, Theme, Renderer>;

pub fn build<'a>(keys: &'a [KeyRow]) -> El<'a> {
    let mut rows: Vec<El<'a>> = vec![text("KEYBOARD BINDINGS").size(16).color(style::ACCENT).into()];
    for k in keys {
        let right: El<'a> = if k.editable {
            let id = k.id.clone();
            let id2 = k.id.clone();
            // Toggle = enabled: off writes an empty combo (disabled), on restores the default.
            let enable = toggler(!k.combo.is_empty())
                .on_toggle(move |on| if on { SettingsMessage::ResetBind(id2.clone()) } else { SettingsMessage::Rebind(id2.clone(), String::new()) })
                .style(control::toggler);
            row![
                enable,
                text_input(&k.default, &k.combo).width(Length::Fixed(160.0)).on_input(move |s| SettingsMessage::Rebind(id.clone(), s)),
                button(text("↺").size(12)).on_press(SettingsMessage::ResetBind(k.id.clone())).style(control::action),
            ].spacing(6).align_y(Alignment::Center).into()
        } else {
            container(text(k.combo.clone()).size(12).color(style::ACCENT)).padding(Padding::from([3, 9])).style(style::chip).into()
        };
        let line = row![text(k.label.clone()).width(Length::Fill), right].align_y(Alignment::Center).spacing(8).padding(Padding::from([6, 12]));
        rows.push(container(line).style(style::card).width(Length::Fill).into());
    }
    // Keep the title + cards as one width-capped column pinned to the right of the
    // pane, so the "KEYBOARD BINDINGS" title sits above the cards (not to their left).
    let panel = container(Column::with_children(rows).spacing(8)).max_width(480.0);
    scrollable(container(panel).width(Length::Fill).align_x(Horizontal::Right)).height(Length::Fill).into()
}
