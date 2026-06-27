//! Wi-Fi tab: on/off, scan, network list; secured networks prompt for a password.
use compositor_configurator_network_backend_base::base::WifiSnapshot;
use compositor_support_iced_core_engine_base::Renderer;
use compositor_configurator_settings_surface_message::message::SettingsMessage;
use compositor_configurator_settings_surface_style::style;
use compositor_configurator_settings_surface_control::control;
use iced_core::{Element, Length, Theme};
use iced_widget::{button, row, scrollable, text, text_input, Column};

pub fn build<'a>(w: &'a WifiSnapshot, selected: Option<&'a str>, password: &'a str) -> Element<'a, SettingsMessage, Theme, Renderer> {
    let header = row![
        text("Wi-Fi").size(18).width(Length::Fill),
        button(text(if w.enabled { "On" } else { "Off" })).on_press(SettingsMessage::WifiEnable(!w.enabled)).style(control::action),
        button(text("Scan")).on_press(SettingsMessage::WifiScan).style(control::action),
    ].spacing(8);
    let mut rows: Vec<Element<'a, SettingsMessage, Theme, Renderer>> = vec![header.into()];
    if !w.available {
        rows.push(text("NetworkManager unavailable.").into());
    }
    for n in &w.networks {
        let lock = if n.secured { "[*] " } else { "" };
        let act = if n.active { "  (connected)" } else { "" };
        let label = format!("{lock}{} - {}%{act}", n.ssid, n.strength);
        if selected == Some(n.ssid.as_str()) && n.secured {
            let ssid = n.ssid.clone();
            rows.push(
                row![
                    text(label).width(Length::Fill),
                    text_input("password", password).width(Length::Fixed(140.0)).on_input(SettingsMessage::WifiPassword),
                    button(text("Join")).on_press(SettingsMessage::WifiConnect(ssid, password.to_string())).style(control::accent),
                ].spacing(8).into(),
            );
        } else {
            let ssid = n.ssid.clone();
            let msg = if n.secured { SettingsMessage::WifiSelect(ssid) } else { SettingsMessage::WifiConnect(ssid, String::new()) };
            rows.push(button(text(label)).width(Length::Fill).on_press(msg).style(control::action).into());
        }
    }
    scrollable(Column::with_children(rows).spacing(6).padding(4)).into()
}
