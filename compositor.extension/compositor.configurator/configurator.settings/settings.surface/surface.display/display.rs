//! Display module: preferred-monitor picker + the selected monitor's advertised
//! modes. A **CHECK CHANGES** button (below the monitor list) provisionally
//! applies the selected monitor/mode through the fault gate; **APPLY** keeps it
//! and **REVERT** undoes it. All three are always rendered (no layout shift) and
//! enabled only when relevant. Selecting a different monitor + applying switches
//! the active output; selecting another mode changes the active monitor's mode.
use compositor_orchestration_driver_output_base::base::{DisplayInfo, ModeInfo};
use compositor_support_iced_core_engine_base::Renderer;
use compositor_configurator_settings_surface_message::message::{Applied, SettingsMessage};
use compositor_configurator_settings_surface_style::style;
use compositor_configurator_settings_surface_control::control;
use iced_core::{Element, Length, Theme};
use iced_widget::{button, column, row, scrollable, text, Column};

type El<'a> = Element<'a, SettingsMessage, Theme, Renderer>;

fn mode_label(m: &ModeInfo) -> String {
    format!("{}×{}   ·   {:.2} Hz", m.width, m.height, m.refresh_mhz as f32 / 1000.0)
}

pub fn build<'a>(
    displays: &[DisplayInfo],
    active_edid: &str,
    selected_display: &str,
    selected_mode: Option<ModeInfo>,
    confirming: bool,
    pending: Option<&Applied>,
) -> El<'a> {
    let head = column![
        text("DISPLAY").size(16).color(style::ACCENT),
        text("Preferred monitor, resolution and refresh rate.").size(11).color(style::MUTED),
    ].spacing(4);

    if displays.is_empty() {
        return column![
            head,
            text("No monitors detected (running nested / no DRM backend).").size(12).color(style::MUTED)
        ].spacing(14).into();
    }

    // Monitor picker (● = active output, accent = selected in the picker).
    let mut monitors: Vec<El<'a>> = vec![text("MONITOR").size(11).color(style::MUTED).into()];
    for d in displays {
        let mark = if d.edid_key == active_edid { "●" } else { "○" };
        let label = format!("{mark}  {}   ·   {}", d.name, d.edid_key);
        let on = d.edid_key == selected_display;
        let b = button(text(label)).width(Length::Fill).on_press(SettingsMessage::SelectDisplay(d.edid_key.clone()));
        monitors.push(if on { b.style(control::accent) } else { b.style(control::action) }.into());
    }

    // Action row directly below the monitor list — always present, conditionally
    // enabled (a button with no on_press is disabled, so the layout never shifts).
    let active = displays.iter().find(|d| d.edid_key == active_edid);
    let changed = selected_display != active_edid
        || (selected_mode.is_some() && selected_mode != active.and_then(|d| d.current));
    // CHECK is enabled only when a change is pending (different monitor OR a
    // different resolution/Hz) AND nothing is currently provisioning. APPLY/REVERT
    // are enabled only WHILE provisioning. A disabled button gets no `on_press` and
    // the greyed `disabled` style so it both looks and behaves inert.
    let check_enabled = !confirming && changed && selected_mode.is_some();
    let check = {
        let b = button(text("CHECK CHANGES")).width(Length::Fill);
        match (check_enabled, selected_mode) {
            (true, Some(mode)) => b.style(control::action).on_press(SettingsMessage::Apply(Applied {
                edid_key: selected_display.to_string(),
                mode,
                switch: selected_display != active_edid,
            })),
            _ => b.style(control::disabled),
        }
    };
    let apply = {
        let b = button(text("APPLY")).width(Length::Fill);
        match (confirming, pending) {
            (true, Some(p)) => b.style(control::accent).on_press(SettingsMessage::Keep(p.clone())),
            _ => b.style(control::disabled),
        }
    };
    let revert = {
        let b = button(text("REVERT")).width(Length::Fill);
        if confirming { b.style(control::action).on_press(SettingsMessage::Revert) } else { b.style(control::disabled) }
    };
    let actions = row![check, apply, revert].spacing(8);

    // Modes for the selected monitor.
    let sel = displays.iter().find(|d| d.edid_key == selected_display);
    let mut modes: Vec<El<'a>> = vec![text("RESOLUTION").size(11).color(style::MUTED).into()];
    match sel {
        Some(d) if !d.available.is_empty() => {
            for m in &d.available {
                let on = Some(*m) == selected_mode;
                let b = button(text(mode_label(m))).width(Length::Fill).on_press(SettingsMessage::SelectMode(*m));
                modes.push(if on { b.style(control::accent) } else { b.style(control::action) }.into());
            }
        }
        _ => modes.push(text("No advertised modes for this monitor.").size(12).color(style::MUTED).into()),
    }

    column![
        head,
        Column::with_children(monitors).spacing(6),
        actions,
        scrollable(Column::with_children(modes).spacing(6)).height(Length::Fill),
    ].spacing(14).into()
}
