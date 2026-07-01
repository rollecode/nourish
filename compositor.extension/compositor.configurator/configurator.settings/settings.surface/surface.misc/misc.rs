//! The Misc module: the input method the compositor launches (preferences.json
//! `ime`). Edit its executable and argument list; each edit emits the whole `Ime`,
//! persisted live to preferences.json by the handler. Empty exec ⇒ y5 launches no
//! input method. Takes effect on the next compositor start.
use compositor_developer_environment_preference_base::base::Ime;
use compositor_support_iced_core_engine_base::Renderer;
use compositor_configurator_settings_surface_message::message::SettingsMessage;
use compositor_configurator_settings_surface_style::style;
use compositor_configurator_settings_surface_control::control;
use iced_core::{Alignment, Element, Length, Theme};
use iced_widget::{button, column, container, row, scrollable, text, text_input, Column};

type El<'a> = Element<'a, SettingsMessage, Theme, Renderer>;

fn card<'a>(inner: El<'a>) -> El<'a> {
    container(inner).style(style::card).width(Length::Fill).into()
}

pub fn build<'a>(ime: &'a Ime) -> El<'a> {
    let head = column![
        text("MISC").size(16).color(style::ACCENT),
        text("Input method launched by the compositor — applied on next start.").size(11).color(style::MUTED),
    ].spacing(4);

    // Executable (empty = no input method). The field owns a clone of the whole `Ime`
    // and re-emits it with `exec` replaced.
    let base = ime.clone();
    let exec_field = text_input("e.g. fcitx5 (empty = no input method)", &ime.exec)
        .width(Length::Fixed(280.0))
        .on_input(move |s| { let mut x = base.clone(); x.exec = s; SettingsMessage::Ime(x) });
    let exec_row = card(
        row![text("Input method exec").width(Length::Fill), exec_field]
            .align_y(Alignment::Center).spacing(10).padding(12).into(),
    );

    let mut rows: Vec<El<'a>> = vec![head.into(), exec_row];

    // Arguments: one editable row per arg with a remove (−), then a trailing add (+).
    // Button messages are computed at view time, so each carries the already-mutated `Ime`.
    rows.push(text("ARGUMENTS").size(10).color(style::MUTED).into());
    for (idx, arg) in ime.args.iter().enumerate() {
        let base = ime.clone();
        let edit = text_input("argument", arg)
            .width(Length::Fill)
            .on_input(move |s| { let mut x = base.clone(); x.args[idx] = s; SettingsMessage::Ime(x) });
        let remove = button(text("−").size(14)).style(control::action)
            .on_press({ let mut x = ime.clone(); x.args.remove(idx); SettingsMessage::Ime(x) });
        rows.push(card(
            row![edit, remove].align_y(Alignment::Center).spacing(10).padding(12).into(),
        ));
    }
    let add = button(text("+ add argument").size(12)).style(control::action)
        .on_press({ let mut x = ime.clone(); x.args.push(String::new()); SettingsMessage::Ime(x) });
    rows.push(add.into());

    scrollable(Column::with_children(rows).spacing(10)).height(Length::Fill).into()
}
