//! Settings-tab embed: the settings iced surface below the menu bar while the
//! Settings tab is active. Replaces the Super+. mount, reusing the UI + driver.
use compositor_monitor_compositor_iced_base::{HandleId, IcedHandle, IcedSpace};
use compositor_orchestration_core_state_base::Loop;
use compositor_orchestration_draw_layer_base::base::Layer;
use compositor_orchestration_driver_audio_base::base::AUDIO;
use compositor_orchestration_driver_output_base::base::OUTPUT_MODES_SNAPSHOT;
use compositor_orchestration_driver_settings_base::base::{SETTINGS, SETTINGS_MUT};
use compositor_configurator_network_backend_base::base::{self as wifi, WifiCmd, WifiSnapshot};
use compositor_configurator_bluetooth_backend_base::base::{self as bt, BtCmd, BtSnapshot};
use compositor_configurator_settings_surface_message::message::SettingsMessage;
use compositor_configurator_settings_surface_view::Settings;
use compositor_y5_audio_controller_interface::interface::AudioState;
use compositor_y5_surface_draw_handle::handle::load;
use compositor_y5_surface_protocol_base::protocol::{SurfaceMessage, SurfaceMessageType};
use compositor_y5_overview_state_base::base::MENU_BAR_HEIGHT;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::utils::{Physical, Point, Rectangle, Size};
use std::cell::RefCell;

thread_local! {
    /// Last snapshot pushed — only re-dispatch (re-render) the UI when it changes.
    static LAST: RefCell<Option<(AudioState, WifiSnapshot, BtSnapshot)>> = const { RefCell::new(None) };
}

pub fn per_frame(state: &mut Loop, renderer: &mut GlesRenderer, size: Size<i32, Physical>) {
    let shown = state.inner.overview().visible
        && state.inner.overview().overlay_ready()
        && state.inner.overview().is_settings();
    match (shown, state.inner.kernel.get(&SETTINGS).handle, state.inner.kernel.get(&SETTINGS).open) {
        (true, None, _) => create(state, renderer, size),
        (true, Some(id), false) => { destroy(state, id); compositor_y5_overview_interface_base::base::request_close(state); } // panel Close
        (true, Some(id), true) => sync(state, id),
        (false, Some(id), _) => destroy(state, id),
        (false, None, _) => {}
    }
}

fn sync(state: &mut Loop, id: HandleId) {
    let audio = state.inner.kernel.get(&AUDIO).as_ref().map(|a| a.state()).unwrap_or_default();
    let cur = (audio, wifi::snapshot(), bt::snapshot());
    let changed = LAST.with(|l| { let mut l = l.borrow_mut(); if l.as_ref() != Some(&cur) { *l = Some(cur.clone()); true } else { false } });
    if changed {
        if let Some(reg) = state.inner.surface_mut().registry.as_mut() {
            let _ = reg.dispatch_message(IcedHandle::<Settings>::from_id(id), SettingsMessage::SyncSystem(cur.0, cur.1, cur.2));
        }
    }
}

fn create(state: &mut Loop, renderer: &mut GlesRenderer, size: Size<i32, Physical>) {
    let rect = Rectangle::new(Point::from((0, MENU_BAR_HEIGHT)), Size::from((size.w, (size.h - MENU_BAR_HEIGHT).max(1))));
    let env = compositor_developer_environment_config_base::base::read_current();
    state.inner.preference = compositor_developer_environment_preference_base::base::load();
    state.inner.keybinding = compositor_developer_environment_keybinding_base::base::load();
    let cursor = state.inner.preference.cursor_sensitivity as f32;
    let natural = state.inner.preference.input_natural_scroll;
    let snap = state.inner.kernel.get(&OUTPUT_MODES_SNAPSHOT).clone();
    let mut keys = compositor_y5_overlay_interface_keyboard::keyboard::registry(&state.inner.keybinding);
    keys.extend(compositor_y5_canvas_input_keyboard::navigator::registry(&state.inner.keybinding));
    keys.extend(compositor_y5_canvas_input_keyboard::navigator::fixed());
    keys.extend(compositor_y5_overlay_interface_keyboard::keyboard::fixed());
    let ui = Settings::new(env, cursor, natural, snap, keys);
    let handle = load(state, renderer, ui, rect, IcedSpace::Screen, Layer::SCENE.bits());
    install_handler(state, handle);
    let untyped = handle.untyped();
    if let Some(reg) = state.inner.surface_mut().registry.as_mut() { reg.set_keyboard_focus(Some(untyped)); }
    let st = state.inner.kernel.get_mut(&SETTINGS_MUT);
    st.handle = Some(untyped);
    st.open = true;
    if let Some(a) = state.inner.kernel.get(&AUDIO) { let _ = a.refresh(); }
    wifi::command(WifiCmd::Scan);
    bt::command(BtCmd::Scan(true));
    LAST.with(|l| *l.borrow_mut() = None);
}

fn destroy(state: &mut Loop, id: HandleId) {
    if let Some(reg) = state.inner.surface_mut().registry.as_mut() { reg.destroy_by_id(id); reg.set_keyboard_focus(None); }
    bt::command(BtCmd::Scan(false));
    let st = state.inner.kernel.get_mut(&SETTINGS_MUT);
    st.handle = None;
    st.open = false;
}

fn install_handler(state: &mut Loop, handle: IcedHandle<Settings>) {
    let tx = state.inner.surface_mut().surface_message_buffer_channel.0.clone();
    if let Some(reg) = state.inner.surface_mut().registry.as_mut() {
        if let Some(inst) = reg.instance_mut(handle) {
            inst.runtime_mut().set_message_handler(move |m: &SettingsMessage| {
                if matches!(m, SettingsMessage::SyncSystem(..) | SettingsMessage::WifiSelect(_) | SettingsMessage::WifiPassword(_)) { return; }
                let _ = tx.send(SurfaceMessage { message: SurfaceMessageType::Settings(m.clone()) });
            });
        }
    }
}
