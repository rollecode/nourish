use compositor_support_system_buffer_token_base::y5_buffer;
use compositor_support_system_channel_token_base::y5_channel;
use compositor_support_system_trait_system_base::base::{BufferCx, System, SystemCx, WorldBuilder};
use compositor_y5_placeholder_state_base::state::PlaceholderState;
use smithay::utils::{Physical, Point, Size};
use std::any::Any;
use uuid::Uuid;

// The slot tokens live with the state (in `placeholder.state`) so the persistence
// document can reference them without a crate cycle; re-export for legacy sites.
pub use compositor_y5_placeholder_state_base::state::{PLACEHOLDER, PLACEHOLDER_MUT};

/// An interactive placeholder geometry update (move/scale drag), migrated from
/// the rim `placeholder.interface::set_visible_geometry`. The placeholder OWNS
/// the slot, so an input system (CanvasSystem) can't write `modify_visible`
/// directly — it announces this; the placeholder system applies the slot half
/// via its buffer AND announces the registry half to the surface system.
/// `position`/`size` are `(x, y)` / `(w, h)` exactly as the rim passed them.
#[derive(Clone, Copy)]
pub struct PlaceholderGeometry {
    pub uuid: Uuid,
    pub position: Option<(i32, i32)>,
    pub size: Option<(i32, i32)>,
}
y5_channel!(pub PLACEHOLDER_GEOMETRY, PLACEHOLDER_GEOMETRY_TX: PlaceholderGeometry);

/// Announce a placeholder geometry change to the placeholder system. Mirrors the
/// surface system's `announce_iced_button`; cross-crate senders can't reach the
/// channel TX directly, so they call this on the world's router.
pub fn announce_placeholder_geometry(
    channels: &mut compositor_support_system_channel_router_base::base::ChannelRouter,
    uuid: Uuid,
    position: Option<(i32, i32)>,
    size: Option<(i32, i32)>,
) {
    channels.send(&PLACEHOLDER_GEOMETRY_TX, PlaceholderGeometry { uuid, position, size });
}

enum PlaceholderCmd {
    SetGeometry(Uuid, Option<(i32, i32)>, Option<(i32, i32)>),
}
y5_buffer!(PLACEHOLDER_BUF: PlaceholderCmd);

/// Owns the placeholder slot.
#[derive(Default)]
pub struct PlaceholderSystem;

impl System for PlaceholderSystem {
    fn name(&self) -> &'static str {
        "placeholder"
    }

    fn register(&mut self, builder: &mut WorldBuilder) {
        builder.storage.insert(&PLACEHOLDER, PlaceholderState::new());
        builder.receive(&PLACEHOLDER_GEOMETRY, Self::on_geometry);
    }

    /// Persist this world's placeholders (slim launch-plan prior data) into the
    /// per-world `world.placeholder` table; rehydrated at world build.
    fn documents(
        &self,
    ) -> &'static [&'static compositor_support_system_persist_document_entry::base::DocumentEntry] {
        PLACEHOLDER_DOCS
    }

    fn buffer(&mut self, cx: &mut BufferCx, message: Box<dyn Any>) {
        match *message.downcast::<PlaceholderCmd>().expect("placeholder buffer type") {
            // Slot half of the rim `set_visible_geometry`: update the visible
            // placeholder's position/size in place. Wrapped in `transact(false, …)`
            // — the geometry is persisted but DEBOUNCED (drags fire many updates/sec,
            // so the disk write batches up to 1s instead of once per frame).
            PlaceholderCmd::SetGeometry(uuid, position, size) => {
                cx.transact(false, |storage| {
                    let state = storage.get_mut(&PLACEHOLDER_MUT);
                    // Visible (window-destroyed) tile — `uuid` is the placeholder id.
                    state.modify_visible(&uuid, |ph| {
                        if let Some((w, h)) = size {
                            ph.size.0 = w;
                            ph.size.1 = h;
                        }
                        if let Some((x, y)) = position {
                            ph.position.0 = x;
                            ph.position.1 = y;
                        }
                    });
                    // Live (window-backed) map placeholder — `uuid` is the window id.
                    // This is the slot half the rim `_reform` kept in sync via
                    // `placeholder.interface::set`; without it the tile spawns at the
                    // pre-drag geometry when the window later closes. The two uuid
                    // namespaces don't collide, so exactly one of these matches.
                    state.modify_present(&uuid, |ph| {
                        if let Some((w, h)) = size {
                            ph.size.0 = w;
                            ph.size.1 = h;
                        }
                        if let Some((x, y)) = position {
                            ph.position.0 = x;
                            ph.position.1 = y;
                        }
                    });
                });
            }
        }
    }
}

impl PlaceholderSystem {
    /// Announced by CanvasSystem during a placeholder move/scale drag. Apply the
    /// slot half via the buffer; resolve the iced handle from the slot and
    /// announce the registry half to the surface system (rim parity:
    /// `set_visible_geometry` resized/relocated the iced surface too).
    fn on_geometry(&mut self, cx: &mut SystemCx, ev: &PlaceholderGeometry) {
        // Resolve the iced handle id for this placeholder (registry half target).
        let handle = cx.storage.get(&PLACEHOLDER).visible.iter().find_map(|w| {
            if w.0.uuid == ev.uuid { Some(w.1.id) } else { None }
        });

        cx.write(&PLACEHOLDER_BUF, PlaceholderCmd::SetGeometry(ev.uuid, ev.position, ev.size));

        if let Some(handle) = handle {
            let position = ev.position.map(|(x, y)| Point::<i32, Physical>::from((x, y)));
            let size = ev.size.map(|(w, h)| Size::<i32, Physical>::from((w, h)));
            compositor_y5_surface_system_base::base::announce_placeholder_geometry(
                cx.channels, handle, position, size,
            );
        }
    }
}

static PLACEHOLDER_DOCS: &[&compositor_support_system_persist_document_entry::base::DocumentEntry] =
    &[&compositor_y5_placeholder_persist_doc::base::PLACEHOLDER_DOC];
