//! Overview menu-bar surface lifecycle + tab actions (the iced message handler,
//! cycle-tab). `interface.base::handle` calls `open`/`close` here.

use smithay::backend::renderer::gles::GlesRenderer;
use smithay::utils::{Point, Rectangle, Size};
use std::sync::mpsc::Sender;
use compositor_orchestration_core_state_base::Loop;
use compositor_monitor_compositor_iced_base::IcedHandle;
use compositor_monitor_overview_ui_base::base::{OverviewMenu, OverviewMessage, Section};
use compositor_y5_overview_state_base::base::{OverviewSurfaceMessage, Tab, MENU_BAR_HEIGHT};
use compositor_y5_surface_protocol_base::protocol::{SurfaceMessage, SurfaceMessageType};

fn section_of(tab: Tab) -> Section {
    match tab {
        Tab::World => Section::World,
        Tab::Layout => Section::Layout,
        Tab::Settings => Section::Settings,
    }
}

/// Super+Left/Right: cycle the active tab and sync the menu-bar highlight.
pub fn cycle_tab(state: &mut Loop, forward: bool) {
    let order = [Tab::World, Tab::Layout, Tab::Settings];
    let idx = order.iter().position(|t| *t == state.inner.overview().tab).unwrap_or(1);
    let next = order[if forward { (idx + 1) % 3 } else { (idx + 2) % 3 }];
    state.inner.overview_mut().tab = next;
    if let Some(id) = state.inner.overview().menu {
        if let Some(reg) = state.inner.surface_mut().registry.as_mut() {
            let _ = reg.dispatch_message(IcedHandle::<OverviewMenu>::from_id(id), OverviewMessage::Select(section_of(next)));
        }
    }
}

/// Create the top menu-bar screen surface (idempotent).
pub fn open(state: &mut Loop, renderer: &mut GlesRenderer) {
    if state.inner.overview().menu.is_some() {
        return;
    }

    let screen = {
        // Size the menu bar for the ACTIVE monitor (the one the user is on), not the
        // primary — screen-space surfaces live on the active output.
        let output = state.inner.active_output();
        output.current_mode().unwrap_or_else(|| abort!("output has a mode")).size
    };

    let rect = Rectangle::new(Point::new(0, 0), Size::new(screen.w, MENU_BAR_HEIGHT));
    let handle = compositor_y5_surface_draw_handle::handle::load(
        state,
        renderer,
        OverviewMenu::new(std::env::var("USER").unwrap_or_else(|_| "user".to_string())),
        rect,
        compositor_y5_surface_draw_handle::handle::IcedSpace::Screen,
        compositor_orchestration_draw_layer_base::base::Layer::SCENE.bits(),
    );

    let tx = state.inner.surface_mut().surface_message_buffer_channel.0.clone();
    state
        .inner
        .surface_mut()
        .registry
        .as_mut()
        .unwrap_or_else(|| abort!("surface registry"))
        .instance_mut(handle)
        .unwrap_or_else(|| abort!("overview menu instance"))
        .runtime_mut()
        .set_message_handler(move |message: &OverviewMessage| dispatch(message, &tx));

    state.inner.overview_mut().menu = Some(handle.id);

    // Explicitly sync the menu-bar highlight to the active tab — it persists for
    // the session, but a freshly-created `OverviewMenu` defaults to Layout.
    let tab = state.inner.overview().tab;
    if let Some(reg) = state.inner.surface_mut().registry.as_mut() {
        let _ = reg.dispatch_message(IcedHandle::<OverviewMenu>::from_id(handle.id), OverviewMessage::Select(section_of(tab)));
    }
}

pub fn close(state: &mut Loop) {
    if let Some(id) = state.inner.overview_mut().menu.take() {
        if let Some(registry) = state.inner.surface_mut().registry.as_mut() {
            registry.destroy_by_id(id);
        }
    }
}

fn dispatch(message: &OverviewMessage, tx: &Sender<SurfaceMessage>) {
    let section = match message {
        OverviewMessage::Select(s) => s,
        OverviewMessage::Clock(_) => return,
    };
    let tab = match section {
        Section::World => Tab::World,
        Section::Layout => Tab::Layout,
        Section::Settings => Tab::Settings,
    };
    let _ = tx.send(SurfaceMessage {
        message: SurfaceMessageType::Overview(OverviewSurfaceMessage::SetTab(tab)),
    });
}
