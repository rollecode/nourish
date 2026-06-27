//! Display module: advertised resolution/refresh modes (pick to apply; the
//! Keep/Revert confirm bar lives in the chrome). FPS lives on the Performance tab.
use compositor_orchestration_driver_output_base::base::ModeInfo;
use compositor_support_iced_core_engine_base::Renderer;
use compositor_configurator_settings_surface_message::message::SettingsMessage;
use compositor_configurator_settings_surface_style::style;
use compositor_configurator_settings_surface_control::control;
use iced_core::{Element, Length, Theme};
use iced_widget::{button, column, scrollable, text, Column};

type El<'a> = Element<'a, SettingsMessage, Theme, Renderer>;

pub fn build<'a>(modes: &[ModeInfo], current: Option<ModeInfo>) -> El<'a> {
    let head = column![
        text("DISPLAY").size(16).color(style::ACCENT),
        text("Output resolution and refresh rate.").size(11).color(style::MUTED),
    ].spacing(4);
    if modes.is_empty() {
        return column![head, text("No advertised modes (running nested / no DRM backend).").size(12).color(style::MUTED)]
            .spacing(14).into();
    }
    let mut rows: Vec<El<'a>> = vec![text("RESOLUTION").size(11).color(style::MUTED).into()];
    for m in modes {
        let on = Some(*m) == current;
        let label = format!("{}×{}   ·   {:.2} Hz", m.width, m.height, m.refresh_mhz as f32 / 1000.0);
        let b = button(text(label)).width(Length::Fill).on_press(SettingsMessage::PickMode(*m));
        rows.push(if on { b.style(control::accent) } else { b.style(control::action) }.into());
    }
    column![head, scrollable(Column::with_children(rows).spacing(6)).height(Length::Fill)].spacing(14).into()
}
