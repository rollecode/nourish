//! Full-width "SYSTEM CONFIGURATION" chrome: title bar, left module sidebar,
//! scrollable section content, and a bottom status/apply bar — built from plain
//! data so the `IcedUi` owner stays tiny. Section bodies live in surface.* builders.
use compositor_developer_environment_config_base::base::Environment;
use compositor_developer_environment_keybinding_base::base::KeyRow;
use compositor_configurator_hardware_gpu_base::base::RenderDevice;
use compositor_orchestration_driver_output_base::base::ModeInfo;
use compositor_support_iced_core_engine_base::Renderer;
use compositor_y5_audio_controller_interface::interface::AudioState;
use compositor_configurator_network_backend_base::base::WifiSnapshot;
use compositor_configurator_bluetooth_backend_base::base::BtSnapshot;
use compositor_configurator_settings_surface_display::display;
use compositor_configurator_settings_surface_cursor::cursor;
use compositor_configurator_settings_surface_keys::keys as keybinds;
use compositor_configurator_settings_surface_environment::environment;
use compositor_configurator_audio_tab_base::base as audio_tab;
use compositor_configurator_network_tab_base::base as network_tab;
use compositor_configurator_bluetooth_tab_base::base as bluetooth_tab;
use compositor_configurator_settings_surface_message::message::{SettingsMessage, Tab};
use compositor_configurator_settings_surface_style::style;
use compositor_configurator_settings_surface_control::control;
use iced_core::{Alignment, Element, Length, Padding, Theme};
use iced_widget::{button, column, container, row, text};

type El<'a> = Element<'a, SettingsMessage, Theme, Renderer>;

fn fixed(px: f32) -> Length {
    Length::Fixed(px)
}

fn module<'a>(icon: &'a str, label: &'a str, t: Tab, sel: Tab) -> El<'a> {
    button(row![text(icon), text(label).size(13)].spacing(10).align_y(Alignment::Center))
        .width(Length::Fill).padding(Padding::from([8, 14]))
        .on_press(SettingsMessage::Tab(t)).style(control::sidebar_item(sel == t)).into()
}

fn sidebar<'a>(sel: Tab) -> El<'a> {
    let list = column![
        text("CONFIG MODULES").size(10).color(style::MUTED),
        module("▦", "DISPLAY", Tab::Display, sel),
        module("♪", "AUDIO", Tab::Audio, sel),
        module("⌨", "INPUT", Tab::Input, sel),
        module("≋", "NETWORK", Tab::Network, sel),
        module("❖", "BLUETOOTH", Tab::Bluetooth, sel),
        module("▲", "PERFORMANCE", Tab::Performance, sel),
        module("⚙", "SYSTEM", Tab::System, sel),
    ].spacing(4).padding(14);
    container(list).width(fixed(224.0)).height(Length::Fill).style(style::sidebar).into()
}

fn titlebar<'a>(dirty: bool) -> El<'a> {
    let sub = if dirty { "y5 COMPOSITOR · REBOOT TO APPLY SOME CHANGES" } else { "y5 COMPOSITOR · RUNTIME CONFIG" };
    let title = column![
        text("SYSTEM CONFIGURATION").size(20),
        text(sub).size(10).color(style::MUTED),
    ].spacing(3);
    container(title).style(style::strip).width(Length::Fill).padding(Padding::from([14, 22])).into()
}

fn performance<'a>(fps: u32) -> El<'a> {
    let cell = container(row![text("FRAME RATE").color(style::MUTED).width(Length::Fill), text(format!("{fps} FPS")).color(style::ACCENT)].align_y(Alignment::Center).padding(16))
        .style(style::card).width(Length::Fill);
    column![text("PERFORMANCE").size(16).color(style::ACCENT), text("Live runtime metrics.").size(11).color(style::MUTED), cell].spacing(12).into()
}

fn confirm<'a>(p: ModeInfo) -> El<'a> {
    let bar = row![
        text("Keep new display mode?").width(Length::Fill),
        button(text("KEEP")).on_press(SettingsMessage::Keep(p)).style(control::accent),
        button(text("REVERT")).on_press(SettingsMessage::Revert).style(control::action),
    ].spacing(10).align_y(Alignment::Center).padding(12);
    container(bar).style(style::card).into()
}

#[allow(clippy::too_many_arguments)]
pub fn render<'a>(
    tab: Tab, dirty: bool, cursor_sensitivity: f32, natural: bool, env: &'a Environment,
    modes: &'a [ModeInfo], current: Option<ModeInfo>, picked: Option<ModeInfo>, confirming: bool,
    keys: &'a [KeyRow], audio: &'a AudioState, wifi: &'a WifiSnapshot, bt: &'a BtSnapshot,
    wifi_selected: Option<&'a str>, wifi_password: &'a str, devices: &'a [RenderDevice], fps: u32,
) -> El<'a> {
    let body: El<'a> = match tab {
        Tab::Display => display::build(modes, current),
        Tab::Audio => audio_tab::build(audio),
        Tab::Input => row![
            container(cursor::build(cursor_sensitivity, natural)).width(Length::FillPortion(3)).height(Length::Fill),
            container(keybinds::build(keys)).width(Length::FillPortion(2)).height(Length::Fill),
        ].spacing(24).height(Length::Fill).into(),
        Tab::Network => network_tab::build(wifi, wifi_selected, wifi_password),
        Tab::Bluetooth => bluetooth_tab::build(bt),
        Tab::Performance => performance(fps),
        Tab::System => environment::build(env, devices),
    };
    // No outer scrollable — each section scrolls its own lists independently.
    let mut content = column![body].spacing(16).height(Length::Fill);
    if confirming {
        if let Some(p) = picked { content = content.push(confirm(p)); }
    }
    let main = row![sidebar(tab), container(content).width(Length::Fill).height(Length::Fill).padding(24)].height(Length::Fill);
    container(column![titlebar(dirty), main]).width(Length::Fill).height(Length::Fill).style(style::backdrop).into()
}
