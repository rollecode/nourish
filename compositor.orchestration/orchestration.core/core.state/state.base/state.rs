use compositor_y5_audio_controller_interface::interface::AudioController;
use compositor_y5_audio_controller_interface::media::MediaController;
use compositor_orchestration_environment_type_base::base::Environment;
use smithay::backend::drm::{DrmDeviceFd, DrmNode};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::multigpu::GpuManager;
use smithay::backend::renderer::multigpu::gbm::GbmGlesBackend;
use smithay::desktop::{Window, layer_map_for_output};
use compositor_y5_graphic_capture_registry::CaptureRegistry;
use compositor_y5_lock_state_base::state::LockState;

use crate::Loop;
use smithay::reexports::calloop::{EventLoop, LoopSignal, RegistrationToken};
use smithay::reexports::wayland_server::DisplayHandle;
use std::cell::RefCell;
use std::ffi::OsString;
use std::rc::Rc;
use std::time::Instant;
use compositor_introspection_sampler_window_base::sampler::SampleResult;
use compositor_y5_camera_state_base::state::Camera;
use compositor_y5_canvas_state_base::state::CanvasState;
use compositor_support_smithay_dispatch_wire_base::wire::Wire;
use compositor_support_smithay_dispatch_wire_trait::wire_trait::WireTrait;

pub struct Loader {
    pub socket_name: OsString,
    // pub socket_name_proprietary: OsString,
    pub display_handle: DisplayHandle,
    pub loop_signal: LoopSignal,
}

/// Deferred world-selection-screen request, drained in the GLES prepare phase
/// (where a renderer + the capture registry are available).
/// Opening is deferred so the current world's framebuffer can be snapshotted for
/// its thumbnail before we switch away from it.
#[derive(Clone, Copy)]
pub enum SetPickerRequest {
    Open,
}
/// The loop object must implement various things like decoration, "resize and movement" modes, and input management.
/// The trait must be implemented in this crate and can be delegated if necessary.
/// While the per-region render loop draws one viewport pane (split / floating),
/// this overrides the focus accessors: `camera()`/`camera_mut()`/`size_context()`
/// resolve to THIS pane's slot + region rect instead of the focused slot + full
/// output. `None` everywhere outside that loop (normal full-output rendering and
/// all input/logic paths). This is the single seam that lets one world render
/// through several cameras into several screen regions in one frame.
#[derive(Clone, Copy)]
pub struct RenderTarget {
    pub slot: compositor_y5_viewport_state_base::state::SlotId,
    /// Region top-left in logical screen pixels.
    pub origin_logical: (f64, f64),
    /// Region size in physical pixels.
    pub size_physical: (f64, f64),
}

/// An in-progress separator drag (resizing two adjacent split slots). Holds the
/// two slots, the divide axis, and the start geometry so motion can redistribute
/// their `weight`s. `None` when no separator is being dragged.
#[derive(Clone, Copy)]
pub struct SeparatorDrag {
    pub a: compositor_y5_viewport_state_base::state::SlotId,
    pub b: compositor_y5_viewport_state_base::state::SlotId,
    pub axis: compositor_y5_viewport_state_base::state::Axis,
    /// Physical cursor coord along the divide axis at drag start.
    pub start_along: f64,
    /// Physical lengths of slots a and b along the axis at drag start.
    pub a_len: f64,
    pub b_len: f64,
    /// `weight(a) + weight(b)` at drag start (preserved; only the split moves).
    pub sum_weight: f64,
}

/// An in-progress floating-pane drag: move (`Super`-drag near an edge) or resize
/// (`Super+Shift`-drag near an edge). `index` is the floating pane; the edge bools
/// fix which sides move on resize.
#[derive(Clone, Copy)]
pub struct FloatingDrag {
    pub index: usize,
    pub resize: bool,
    pub left: bool,
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub start_cursor: (f64, f64),
    pub start_rect: smithay::utils::Rectangle<i32, smithay::utils::Physical>,
}

pub struct Orchestrator {
    pub start_time: std::time::Instant,
    /// Active per-region render override (see [`RenderTarget`]). Set only inside
    /// the `scene.frame` region loop.
    pub render_target: Option<RenderTarget>,
    /// In-progress viewport separator drag (resize), if any.
    pub separator_drag: Option<SeparatorDrag>,
    /// In-progress floating-pane move/resize drag, if any.
    pub floating_drag: Option<FloatingDrag>,
    pub status: Status,
    /// One-shot request to run the renderer-free lock engage (`lock_logical`) off
    /// the render loop. The lock keybinding sets `Status::Locked` synchronously and
    /// flips this; `wire.input` drains it and schedules the engage on an idle (the
    /// keyboard crates can't call `lock.interface` — it depends back on them).
    pub lock_engage: bool,
    // Deferred request to open the world-selection screen on a coming draw.
    pub __set_picker: Option<SetPickerRequest>,
    pub status_session: StatusSession,
    /// Per-seat touchpad swipe accumulator (libinput Begin→Update*→End spans
    /// multiple input dispatches). World-agnostic raw delta; the y5 gesture
    /// handler turns it into a directional-view action at end-of-swipe.
    pub gesture: compositor_orchestration_seat_gesture_state::state::GestureAccumulator,
    pub loader: Loader,
    /// The world set (phase 3, document/ARCHITECTURE.md). The active world
    /// hosts the kernel systems; grows per-output/lock/selection worlds later.
    pub worlds: compositor_orchestration_world_manager_base::manager::WorldManager,
    /// KernelData: smithay wiring handles behind storage tokens (read-only for
    /// systems; populated post-init by the loader via smithay.data populate()).
    pub kernel: compositor_support_system_storage_slot_base::base::Storage,
    /// TRANSITIONAL legacy bus: deferred messages whose receivers still need
    /// the whole Loop. Dies with this struct (phase 6).
    pub bus: compositor_orchestration_bus_legacy_base::legacy::LegacyBus<crate::Loop>,
    pub pilot_tick: u64,
    // rpc is now driver data in `kernel` (compositor_orchestration_driver_remote_base).
    // sampler is now driver data in `kernel` (compositor_orchestration_driver_introspection_base).
    pub storage: compositor_orchestration_storage_state_base::state::Storage,

    // __gpu_ref is now driver data in `kernel` (GPU_BINDING token, above).
    // capture_registry/capture are now driver data in `kernel`
    // (compositor_orchestration_driver_capture_base CAPTURE_REGISTRY / CAPTURE).

    // audio/media are now driver data in `kernel` (compositor_orchestration_driver_audio_base).
    pub environment: Environment,
    /// Live user preferences (cursor speed, touchpad natural-scroll, per-EDID
    /// output modes) — the inline-reloaded counterpart to the read-once
    /// `environment`. Seeded at startup from `preference::load()`, refreshed from
    /// disk whenever the settings window opens, and written live by the settings
    /// handler (which then persists it). Read per-event by motion.rs / axis.rs;
    /// reachable from both the input path and the UI handler via `&mut Loop`.
    pub preference: compositor_developer_environment_preference_base::base::Preference,
    /// Live keyboard-shortcut overrides (keybinding.json). Seeded at startup,
    /// refreshed whenever the settings window opens, written by the settings
    /// handler. Read by the overlay shortcut path on every keypress (parse-or-
    /// default). The inline-reloaded counterpart to the read-once settings.
    pub keybinding: compositor_developer_environment_keybinding_base::base::KeyBindings,
}

pub struct StateDRMBinding {
    pub gpus: GpuManager<GbmGlesBackend<GlesRenderer, DrmDeviceFd>>,
    pub primary: DrmNode,
}

/// GPU driver data: the DRM multi-GPU binding (for dmabuf import) lives in the
/// kernel/driver storage by token, not as an Orchestrator field. `Option` —
/// populated post-init by the active backend (winit/udev). The token type lives
/// here with `StateDRMBinding` to avoid a dependency cycle.
pub static GPU_BINDING: compositor_support_system_storage_token_base::base::Token<Option<Rc<RefCell<StateDRMBinding>>>> =
    compositor_support_system_storage_token_base::base::Token::new();
pub static GPU_BINDING_MUT: compositor_support_system_storage_token_base::base::TokenMut<Option<Rc<RefCell<StateDRMBinding>>>> =
    compositor_support_system_storage_token_base::base::TokenMut::new(&GPU_BINDING);

/// TEMPORARY (sanity test): the world ids behind the Super+Alt+1/2/3 switch
/// shortcuts — slot 0 is the main world, 1/2 are pre-created spatial test worlds.
/// Driver data so the overlay shortcut can resolve them. Removed once real world
/// selection lands.
pub static TEST_WORLDS: compositor_support_system_storage_token_base::base::Token<[uuid::Uuid; 3]> =
    compositor_support_system_storage_token_base::base::Token::new();

pub enum Status {
    Running,

    Locked {
        pending: bool,
        sleep: bool,
        time: Instant,
    },
    Unlock {
        // pending: bool,
        time: Instant,
    },
    Sleep {
        // Always locks on sleep.
        pending: bool,
    },
    Terminate,
}
pub enum StatusSession {
    Active,
    Paused,
}

impl Orchestrator {
    pub fn new(
        environment: Environment,
        nested: bool,
        loader: Loader,

        rpc_broadcast: tokio::sync::broadcast::Sender<
            compositor_remote_message_server_base::message::Message,
        >,
        mut kernel_data: compositor_support_system_storage_slot_base::base::Storage,
        worlds: compositor_orchestration_world_manager_base::manager::WorldManager,
    ) -> Self {
        let start_time = std::time::Instant::now();

        // Live user preferences (cursor speed, touch natural-scroll) loaded fresh
        // from preferences.json to seed the runtime cells below.
        let prefs = compositor_developer_environment_preference_base::base::load();
        // Keyboard-shortcut overrides loaded fresh from keybinding.json.
        let keybinding = compositor_developer_environment_keybinding_base::base::load();

        // Audio/media are driver data: stored in the kernel/driver storage by
        // token, not as Orchestrator fields.
        kernel_data.insert(&compositor_orchestration_driver_audio_base::base::AUDIO, AudioController::new("y5.compositor").ok());
        kernel_data.insert(&compositor_orchestration_driver_audio_base::base::MEDIA, Some(MediaController::new()));
        // Introspection driver: sampler slot, populated by the loader post-init.
        kernel_data.insert(&compositor_orchestration_driver_introspection_base::base::SAMPLER, None);
        // Remote driver: RPC state (broadcast + incoming buffer).
        kernel_data.insert(&compositor_orchestration_driver_remote_base::base::RPC, compositor_remote_message_state_base::state::State::new(rpc_broadcast));
        // GPU driver: DRM binding slot, populated post-init by the backend.
        kernel_data.insert(&GPU_BINDING, None);
        // Resume driver: vblank-seen flag + resume watchdog.
        kernel_data.insert(&compositor_orchestration_driver_resume_base::base::VBLANK_SEEN, false);
        kernel_data.insert(&compositor_orchestration_driver_resume_base::base::RESUME_WATCHDOG, None);
        // Lid/display driver: kernel-written display snapshot + rim-written request.
        kernel_data.insert(&compositor_orchestration_driver_lid_base::base::DISPLAY_SNAPSHOT, Default::default());
        kernel_data.insert(&compositor_orchestration_driver_lid_base::base::LID_POSITION, None);
        kernel_data.insert(&compositor_orchestration_driver_lid_base::base::DISPLAY_REQUEST, None);
        kernel_data.insert(&compositor_orchestration_driver_lid_base::base::DISPLAY_OFF, false);
        // logind driver: power client, populated post-init by the backend.
        kernel_data.insert(&compositor_orchestration_driver_logind_base::base::LOGIND, None);
        // Capture driver: registry + session state.
        kernel_data.insert(&compositor_orchestration_driver_capture_base::base::CAPTURE_REGISTRY, None);
        kernel_data.insert(&compositor_orchestration_driver_capture_base::base::CAPTURE, compositor_y5_graphic_capture_session::session::CaptureState::idle());
        // Backend kind (nested winit vs udev) mirrored into the kernel store so
        // input/draw systems can read it via `cx.kernel`.
        kernel_data.insert(&compositor_orchestration_storage_state_base::state::NESTED, nested);

        // Selection-overlay driver: the align/distribute toolbar instance.
        kernel_data.insert(&compositor_orchestration_driver_selection_base::base::SELECTION_OVERLAY, Default::default());

        // Output-mode driver: rim-issued mode request + kernel-written advertised
        // modes snapshot and apply result (settings window ↔ DRM, like the lid).
        kernel_data.insert(&compositor_orchestration_driver_output_base::base::OUTPUT_MODE_REQUEST, None);
        kernel_data.insert(&compositor_orchestration_driver_output_base::base::OUTPUT_MODES_SNAPSHOT, Default::default());
        kernel_data.insert(&compositor_orchestration_driver_output_base::base::OUTPUT_MODE_RESULT, None);
        // Active-output switch driver: rim-issued switch request + kernel-written
        // full connector list and switch result (preferred-monitor change gate).
        kernel_data.insert(&compositor_orchestration_driver_output_base::base::OUTPUTS_SNAPSHOT, Default::default());
        kernel_data.insert(&compositor_orchestration_driver_output_base::base::OUTPUT_SWITCH_REQUEST, None);
        kernel_data.insert(&compositor_orchestration_driver_output_base::base::OUTPUT_SWITCH_RESULT, None);

        // Settings-window driver: the open/handle/dirty state for the Super+. surface.
        kernel_data.insert(&compositor_orchestration_driver_settings_base::base::SETTINGS, Default::default());

        Self {
            environment,
            render_target: None,
            separator_drag: None,
            floating_drag: None,
            lock_engage: false,
            __set_picker: None,
            status_session: StatusSession::Active,
            gesture: Default::default(),
            storage: compositor_orchestration_storage_state_base::state::Storage::new(nested),
            status: Status::Running,
            start_time,
            loader,
            kernel: kernel_data,
            worlds,
            bus: compositor_orchestration_bus_legacy_base::legacy::LegacyBus::new(),
            pilot_tick: 0,
            // Seed the live preference object from preferences.json (one disk read
            // at startup; refreshed on each settings-window open). Missing file →
            // sane defaults.
            preference: prefs,
            keybinding,
        }
    }

    /// The window `Space` hosted by the spawn-target spatial world (currently
    /// the main world). Space is owned by the world (document/ARCHITECTURE.md →
    /// "Window tracking"); this is the driver-side accessor. Borrows only
    /// `self` (Orchestrator/`inner`), so it stays disjoint from `Wire.state`.
    /// (WT2 generalizes "main" to the tracked spawn-target.)
    pub fn space_state(&self) -> &compositor_support_smithay_state_space_base::state::SpaceState {
        let target = self.worlds.spawn_target();
        &self
            .worlds
            .get(target)
            .storage()
            .get(&compositor_support_world_host_space_base::base::SPACE)
            .inner
    }

    pub fn space_state_mut(&mut self) -> &mut compositor_support_smithay_state_space_base::state::SpaceState {
        let target = self.worlds.spawn_target();
        &mut self
            .worlds
            .get_mut(target)
            .storage_mut()
            .get_mut(&compositor_support_world_host_space_base::base::SPACE_MUT)
            .inner
    }

    /// FOCUS ACCESSOR (document/WORLD_DELEGATION.md): the camera/viewport of the
    /// focused world. The rim must read/write the camera through this — never a
    /// literal world id — so view state follows the active/spawn-target world.
    pub fn camera(&self) -> &compositor_y5_camera_state_base::state::Camera {
        let target = self.worlds.spawn_target();
        let viewports = self.worlds.get(target).storage().get(&compositor_y5_viewport_state_base::state::VIEWPORTS);
        // Inside the per-region render loop, resolve the pane being drawn; else
        // the focused (active) slot.
        match self.render_target {
            Some(rt) => viewports.camera_of(rt.slot).unwrap_or_else(|| viewports.focus_camera()),
            None => viewports.focus_camera(),
        }
    }

    pub fn camera_mut(&mut self) -> &mut compositor_y5_camera_state_base::state::Camera {
        let target = self.worlds.spawn_target();
        let render_slot = self.render_target.map(|rt| rt.slot);
        let viewports = self.worlds.get_mut(target).storage_mut().get_mut(&compositor_y5_viewport_state_base::state::VIEWPORTS_MUT);
        // Render target may be a floating pane's slot, so search all panes.
        match render_slot.filter(|id| viewports.camera_of(*id).is_some()) {
            Some(id) => viewports.camera_of_mut(id).expect("checked present"),
            None => viewports.focus_camera_mut(),
        }
    }

    /// FOCUS ACCESSOR: the focused world's viewport tree (slots + cameras).
    pub fn viewports(&self) -> &compositor_y5_viewport_state_base::state::Viewports {
        let target = self.worlds.spawn_target();
        self.worlds.get(target).storage().get(&compositor_y5_viewport_state_base::state::VIEWPORTS)
    }

    pub fn viewports_mut(&mut self) -> &mut compositor_y5_viewport_state_base::state::Viewports {
        let target = self.worlds.spawn_target();
        self.worlds.get_mut(target).storage_mut().get_mut(&compositor_y5_viewport_state_base::state::VIEWPORTS_MUT)
    }

    /// FOCUS ACCESSOR: the focused world's canvas slot (input grab, …).
    pub fn canvas(&self) -> &compositor_y5_canvas_state_base::state::CanvasState {
        let target = self.worlds.spawn_target();
        self.worlds.get(target).storage().get(&compositor_y5_canvas_system_base::base::CANVAS)
    }

    pub fn canvas_mut(&mut self) -> &mut compositor_y5_canvas_state_base::state::CanvasState {
        let target = self.worlds.spawn_target();
        self.worlds.get_mut(target).storage_mut().get_mut(&compositor_y5_canvas_system_base::base::CANVAS_MUT)
    }

    /// FOCUS ACCESSOR: the focused world's navigator state machine.
    pub fn navigator(&self) -> &compositor_y5_navigator_state_base::state::Machine {
        let target = self.worlds.spawn_target();
        self.worlds.get(target).storage().get(&compositor_y5_navigator_state_base::state::NAVIGATOR)
    }

    pub fn navigator_mut(&mut self) -> &mut compositor_y5_navigator_state_base::state::Machine {
        let target = self.worlds.spawn_target();
        self.worlds.get_mut(target).storage_mut().get_mut(&compositor_y5_navigator_state_base::state::NAVIGATOR_MUT)
    }

    /// FOCUS ACCESSOR: the focused world's channel router — rim triggers announce
    /// here so the focused world's systems receive (replaces get(MAIN_WORLD).channels()).
    pub fn focus_channels(&mut self) -> &mut compositor_support_system_channel_router_base::base::ChannelRouter {
        let target = self.worlds.spawn_target();
        self.worlds.get_mut(target).channels()
    }

    /// FOCUS ACCESSOR: the focused world's window-selection slot.
    pub fn select(&self) -> &compositor_y5_select_state_base::select::CanvasSelect {
        let target = self.worlds.spawn_target();
        self.worlds.get(target).storage().get(&compositor_y5_select_state_base::select::SELECT)
    }

    pub fn select_mut(&mut self) -> &mut compositor_y5_select_state_base::select::CanvasSelect {
        let target = self.worlds.spawn_target();
        self.worlds.get_mut(target).storage_mut().get_mut(&compositor_y5_select_state_base::select::SELECT_MUT)
    }

    /// FOCUS ACCESSOR: the focused world's window-grouping slot.
    pub fn group(&self) -> &compositor_y5_group_state_base::state::GroupState {
        let target = self.worlds.spawn_target();
        self.worlds.get(target).storage().get(&compositor_y5_group_state_base::state::GROUP)
    }

    pub fn group_mut(&mut self) -> &mut compositor_y5_group_state_base::state::GroupState {
        let target = self.worlds.spawn_target();
        self.worlds.get_mut(target).storage_mut().get_mut(&compositor_y5_group_state_base::state::GROUP_MUT)
    }

    /// FOCUS ACCESSOR: the focused world's surface slot (iced registry + the
    /// surface-message channel). Per-world; the shared iced GPU context being
    /// wired only to the main world is a separate concern (a test world's
    /// registry is None until that lands).
    pub fn surface(&self) -> &compositor_y5_surface_state_base::state::SurfaceState {
        let target = self.worlds.spawn_target();
        self.worlds.get(target).storage().get(&compositor_y5_surface_system_base::base::SURFACE)
    }

    pub fn surface_mut(&mut self) -> &mut compositor_y5_surface_state_base::state::SurfaceState {
        let target = self.worlds.spawn_target();
        self.worlds.get_mut(target).storage_mut().get_mut(&compositor_y5_surface_system_base::base::SURFACE_MUT)
    }

    /// FOCUS ACCESSOR: the focused world's overview-mode slot (Super+Tab overlay).
    pub fn overview(&self) -> &compositor_y5_overview_state_base::base::Overview {
        let target = self.worlds.spawn_target();
        self.worlds.get(target).storage().get(&compositor_y5_overview_state_base::base::OVERVIEW)
    }

    pub fn overview_mut(&mut self) -> &mut compositor_y5_overview_state_base::base::Overview {
        let target = self.worlds.spawn_target();
        self.worlds.get_mut(target).storage_mut().get_mut(&compositor_y5_overview_state_base::base::OVERVIEW_MUT)
    }

    /// FOCUS ACCESSOR: the focused world's pointer slot (cursor world coords).
    pub fn pointer(&self) -> &compositor_orchestration_seat_pointer_state::state::PointerState {
        let target = self.worlds.spawn_target();
        self.worlds.get(target).storage().get(&compositor_orchestration_seat_system_pointer::base::POINTER)
    }

    pub fn pointer_mut(&mut self) -> &mut compositor_orchestration_seat_pointer_state::state::PointerState {
        let target = self.worlds.spawn_target();
        self.worlds.get_mut(target).storage_mut().get_mut(&compositor_orchestration_seat_system_pointer::base::POINTER_MUT)
    }

    /// FOCUS ACCESSOR: the focused world's placeholder slot.
    pub fn placeholder(&self) -> &compositor_y5_placeholder_state_base::state::PlaceholderState {
        let target = self.worlds.spawn_target();
        self.worlds.get(target).storage().get(&compositor_y5_placeholder_system_base::base::PLACEHOLDER)
    }

    pub fn placeholder_mut(&mut self) -> &mut compositor_y5_placeholder_state_base::state::PlaceholderState {
        let target = self.worlds.spawn_target();
        self.worlds.get_mut(target).storage_mut().get_mut(&compositor_y5_placeholder_system_base::base::PLACEHOLDER_MUT)
    }

    /// FOCUS ACCESSOR: the focused world's launcher slot.
    pub fn launcher(&self) -> &compositor_y5_launcher_draw_state::state::State {
        let target = self.worlds.spawn_target();
        self.worlds.get(target).storage().get(&compositor_y5_launcher_system_base::base::LAUNCHER)
    }

    pub fn launcher_mut(&mut self) -> &mut compositor_y5_launcher_draw_state::state::State {
        let target = self.worlds.spawn_target();
        self.worlds.get_mut(target).storage_mut().get_mut(&compositor_y5_launcher_system_base::base::LAUNCHER_MUT)
    }

    /// FOCUS ACCESSOR: the focused world's window-lifecycle queue (smithay Wire
    /// pushes map/destroy/fullscreen events here; the focused world's WindowSystem
    /// drains them — new windows map into the focused/spawn-target world).
    pub fn window_lifecycle_mut(&mut self) -> &mut compositor_y5_window_lifecycle_state::lifecycle::WindowLifecycle {
        let target = self.worlds.spawn_target();
        self.worlds.get_mut(target).storage_mut().get_mut(&compositor_y5_window_system_base::base::WINDOW_LIFECYCLE_MUT)
    }

    /// Register a drawable at the top of the draw-order authority. Agnostic:
    /// works for any drawable (windows today; iced surfaces, …). Called from
    /// EVERY map path so `drawable_order()` never drops a live drawable.
    pub fn register_drawable(&mut self, uuid: uuid::Uuid, layer: compositor_support_world_order_track_base::base::DrawLayer) {
        let target = self.worlds.spawn_target();
        self.worlds
            .get_mut(target)
            .storage_mut()
            .get_mut(&compositor_support_world_order_track_base::base::DRAW_ORDER_MUT)
            .insert_top(compositor_support_world_order_track_base::base::ComponentId(uuid), layer);
    }

    /// Raise a drawable to the top of the spatial world's draw-order authority
    /// (windows mirror `space.raise_element`; iced raises on interaction). Lazily
    /// registers if absent. See document/ARCHITECTURE.md → "Window tracking".
    pub fn raise_drawable(&mut self, uuid: uuid::Uuid) {
        let target = self.worlds.spawn_target();
        self.worlds
            .get_mut(target)
            .storage_mut()
            .get_mut(&compositor_support_world_order_track_base::base::DRAW_ORDER_MUT)
            .raise(compositor_support_world_order_track_base::base::ComponentId(uuid));
    }

    /// Unregister a drawable from the draw-order authority (event-driven GC):
    /// called from EVERY destruction path (window unmap, iced surface destroy)
    /// so the order never retains a dead component. Foreign/absent ids are a
    /// no-op (`DrawOrder::remove` retains-by-id).
    pub fn remove_drawable(&mut self, uuid: uuid::Uuid) {
        let target = self.worlds.spawn_target();
        self.worlds
            .get_mut(target)
            .storage_mut()
            .get_mut(&compositor_support_world_order_track_base::base::DRAW_ORDER_MUT)
            .remove(compositor_support_world_order_track_base::base::ComponentId(uuid));
    }

    /// Drawable ids in draw order, TOPMOST-FIRST (matches smithay's
    /// first-is-front element order). Each owner resolves its own ids and skips
    /// the rest (the canvas resolves window uuids via the space).
    pub fn drawable_order(&self) -> Vec<uuid::Uuid> {
        let target = self.worlds.spawn_target();
        self.worlds
            .get(target)
            .storage()
            .get(&compositor_support_world_order_track_base::base::DRAW_ORDER)
            .ordered()
            .iter()
            .rev()
            .map(|(id, _)| id.0)
            .collect()
    }

    /// The spawn-target (spatial) world's raw `Storage` — the read source the rim
    /// hit-test bundles into a `HitCx`. A Pass-1 input system gets the equivalent
    /// as `cx.storage` (its active world), so the same hit-test logic serves both.
    pub fn spatial_storage(&self) -> &compositor_support_system_storage_slot_base::base::Storage {
        self.worlds.get(self.worlds.spawn_target()).storage()
    }
}

pub trait CoordinateTrait {
    /// Full-output projection context: anchored to the whole output's centre.
    /// Use for screen-space content (screen iced surfaces, pointer, layer-shell,
    /// lock/overview/picker) — anything that spans the physical screen, NOT an
    /// individual viewport pane. Always full-output, even inside the per-region
    /// render loop.
    fn size_ctx_all(&self) -> compositor_y5_camera_transform_translate::transform::Context;

    /// Projection context for a SPECIFIC viewport pane (`slot`): its camera
    /// anchored to its on-screen region rect, independent of the render loop /
    /// `render_target`. Use anywhere that must be truly per-viewport (e.g.
    /// snapping to a pane's extent). Falls back to full-output if the slot is gone.
    fn size_ctx_viewport(
        &self,
        slot: compositor_y5_viewport_state_base::state::SlotId,
    ) -> compositor_y5_camera_transform_translate::transform::Context;

    /// Per-viewport projection context. Inside the per-region render loop it
    /// anchors to the pane being drawn (its rect + that slot's camera); outside
    /// the loop it equals [`size_ctx_all`]. Use for world content drawn into a
    /// viewport (windows, decorations, canvas cursor, selection) so it projects
    /// into — and is clipped to — the active pane.
    fn viewport_context(&self) -> compositor_y5_camera_transform_translate::transform::Context;

    /// Resolve which viewport pane the physical cursor `phys` is over, record it
    /// as the focused world's `pointer` slot (so `camera()` / camera systems / the
    /// cursor now operate on THAT pane), and return that pane's region context for
    /// mapping the physical cursor to world coordinates. Used by the pointer input
    /// path so input always follows the pane under the cursor, never the
    /// keyboard-`active` pane.
    fn pointer_context(
        &mut self,
        phys: smithay::utils::Point<f64, smithay::utils::Physical>,
    ) -> compositor_y5_camera_transform_translate::transform::Context;

    /// Read-only region context for the CURRENT pointer pane (the `pointer` slot,
    /// already set by the last motion). Use to project the world pointer location
    /// back to physical for the cursor — outside the per-region render loop, where
    /// `viewport_context` would otherwise fall back to full-output.
    fn focus_pane_context(&self) -> compositor_y5_camera_transform_translate::transform::Context;

    /// If `phys` (physical cursor) is over a separator bar, begin a separator drag
    /// and return true (the caller should consume the press). Else false.
    fn try_begin_separator_drag(&mut self, phys: smithay::utils::Point<f64, smithay::utils::Physical>) -> bool;

    /// While a separator drag is active, redistribute the two slots' `weight`s
    /// from `phys` and return true (consume the motion). Else false.
    fn update_separator_drag(&mut self, phys: smithay::utils::Point<f64, smithay::utils::Physical>) -> bool;

    /// End any in-progress separator drag.
    fn end_separator_drag(&mut self);

    /// If `phys` is near a floating pane's edge, begin a move (`resize=false`) or
    /// resize (`resize=true`) drag and return true (consume). Else false. The pane
    /// interior is left for window interaction.
    fn try_begin_floating_drag(&mut self, phys: smithay::utils::Point<f64, smithay::utils::Physical>, resize: bool) -> bool;

    /// While a floating drag is active, move/resize the pane from `phys`; true = consume.
    fn update_floating_drag(&mut self, phys: smithay::utils::Point<f64, smithay::utils::Physical>) -> bool;

    /// End any in-progress floating drag.
    fn end_floating_drag(&mut self);
}
impl CoordinateTrait for Loop {
    fn size_ctx_all(&self) -> compositor_y5_camera_transform_translate::transform::Context {
        let output = self.inner.space_state().state.outputs().next().unwrap();
        let mode = output.current_mode().unwrap_or_else(|| abort!("output has a current mode"));
        let scale = output.current_scale().fractional_scale();
        let camera = &self.inner.camera().transform;
        compositor_y5_camera_transform_translate::transform::Context::new(
            (camera.position.x, camera.position.y),
            camera.zoom,
            (mode.size.w as f64, mode.size.h as f64),
            scale,
        )
    }

    fn size_ctx_viewport(
        &self,
        slot: compositor_y5_viewport_state_base::state::SlotId,
    ) -> compositor_y5_camera_transform_translate::transform::Context {
        let (mode_w, mode_h, scale) = {
            let output = self.inner.space_state().state.outputs().next().unwrap();
            let mode = output.current_mode().unwrap_or_else(|| abort!("output has a current mode"));
            (mode.size.w, mode.size.h, output.current_scale().fractional_scale())
        };
        let bounds = smithay::utils::Rectangle::new(smithay::utils::Point::from((0, 0)), smithay::utils::Size::from((mode_w, mode_h)));
        let viewports = self.inner.viewports();
        // Slot missing → full output.
        let Some(rect) = compositor_y5_viewport_layout_base::layout::compute(viewports, bounds)
            .regions
            .iter()
            .find(|r| r.slot == slot)
            .map(|r| r.rect)
        else {
            return self.size_ctx_all();
        };
        let camera = viewports.camera_of(slot).map(|c| &c.transform).unwrap_or(&viewports.focus_camera().transform);
        compositor_y5_camera_transform_translate::transform::Context::new_region(
            (camera.position.x, camera.position.y),
            camera.zoom,
            (rect.loc.x as f64 / scale, rect.loc.y as f64 / scale),
            (rect.size.w as f64, rect.size.h as f64),
            scale,
        )
    }

    fn viewport_context(&self) -> compositor_y5_camera_transform_translate::transform::Context {
        // No active pane → full output (identical to `size_ctx_all`).
        let Some(rt) = self.inner.render_target else {
            return self.size_ctx_all();
        };
        let output = self.inner.space_state().state.outputs().next().unwrap();
        let scale = output.current_scale().fractional_scale();
        // `camera()` already resolves to the render-target pane's slot camera.
        let camera = &self.inner.camera().transform;
        compositor_y5_camera_transform_translate::transform::Context::new_region(
            (camera.position.x, camera.position.y),
            camera.zoom,
            rt.origin_logical,
            rt.size_physical,
            scale,
        )
    }

    fn pointer_context(
        &mut self,
        phys: smithay::utils::Point<f64, smithay::utils::Physical>,
    ) -> compositor_y5_camera_transform_translate::transform::Context {
        let (mode_w, mode_h, scale) = {
            let output = self.inner.space_state().state.outputs().next().unwrap();
            let mode = output.current_mode().unwrap_or_else(|| abort!("output has a current mode"));
            (mode.size.w, mode.size.h, output.current_scale().fractional_scale())
        };
        let bounds = smithay::utils::Rectangle::new(
            smithay::utils::Point::from((0, 0)),
            smithay::utils::Size::from((mode_w, mode_h)),
        );
        let computed = compositor_y5_viewport_layout_base::layout::compute(self.inner.viewports(), bounds);
        let p = smithay::utils::Point::<i32, smithay::utils::Physical>::from((phys.x.round() as i32, phys.y.round() as i32));
        // During a viewport drag (separator / floating move-resize), FREEZE the
        // operative pane so the cursor mapping stays put and can't jump between
        // viewports mid-drag. Otherwise: over a leaf → that pane; over a
        // separator/gap → keep the current pane (so the round-trip still lands on
        // the separator for hit-testing and the cursor doesn't jump).
        let dragging = self.inner.separator_drag.is_some() || self.inner.floating_drag.is_some();
        let (slot, rect) = if dragging {
            let current = self.inner.viewports().pointer;
            let rect = computed.regions.iter().find(|reg| reg.slot == current).map(|reg| reg.rect).unwrap_or(bounds);
            (current, rect)
        } else {
            match compositor_y5_viewport_layout_base::layout::slot_at(&computed, p) {
                Some((s, r)) => (s, r),
                None => {
                    let current = self.inner.viewports().pointer;
                    let rect = computed.regions.iter().find(|reg| reg.slot == current).map(|reg| reg.rect).unwrap_or(bounds);
                    (current, rect)
                }
            }
        };
        self.inner.viewports_mut().pointer = slot;
        // `camera()` now resolves to the pointer pane (just set).
        let (cx, cy, cz) = {
            let c = &self.inner.camera().transform;
            (c.position.x, c.position.y, c.zoom)
        };
        compositor_y5_camera_transform_translate::transform::Context::new_region(
            (cx, cy),
            cz,
            (rect.loc.x as f64 / scale, rect.loc.y as f64 / scale),
            (rect.size.w as f64, rect.size.h as f64),
            scale,
        )
    }

    fn focus_pane_context(&self) -> compositor_y5_camera_transform_translate::transform::Context {
        let (mode_w, mode_h, scale) = {
            let output = self.inner.space_state().state.outputs().next().unwrap();
            let mode = output.current_mode().unwrap_or_else(|| abort!("output has a current mode"));
            (mode.size.w, mode.size.h, output.current_scale().fractional_scale())
        };
        let bounds = smithay::utils::Rectangle::new(
            smithay::utils::Point::from((0, 0)),
            smithay::utils::Size::from((mode_w, mode_h)),
        );
        let pointer = self.inner.viewports().pointer;
        let computed = compositor_y5_viewport_layout_base::layout::compute(self.inner.viewports(), bounds);
        let rect = computed.regions.iter().find(|r| r.slot == pointer).map(|r| r.rect).unwrap_or(bounds);
        // `camera()` resolves to the pointer pane (focus_camera) outside the render loop.
        let (cx, cy, cz) = {
            let c = &self.inner.camera().transform;
            (c.position.x, c.position.y, c.zoom)
        };
        compositor_y5_camera_transform_translate::transform::Context::new_region(
            (cx, cy),
            cz,
            (rect.loc.x as f64 / scale, rect.loc.y as f64 / scale),
            (rect.size.w as f64, rect.size.h as f64),
            scale,
        )
    }

    fn try_begin_separator_drag(&mut self, phys: smithay::utils::Point<f64, smithay::utils::Physical>) -> bool {
        use compositor_y5_viewport_state_base::state::Axis;
        let (mode_w, mode_h) = {
            let output = self.inner.space_state().state.outputs().next().unwrap();
            let mode = output.current_mode().unwrap_or_else(|| abort!("output has a current mode"));
            (mode.size.w, mode.size.h)
        };
        let bounds = smithay::utils::Rectangle::new(smithay::utils::Point::from((0, 0)), smithay::utils::Size::from((mode_w, mode_h)));
        let computed = compositor_y5_viewport_layout_base::layout::compute(self.inner.viewports(), bounds);
        let p = smithay::utils::Point::<i32, smithay::utils::Physical>::from((phys.x.round() as i32, phys.y.round() as i32));
        let Some(sep) = compositor_y5_viewport_layout_base::layout::separator_at(&computed, p) else { return false };
        let (a, b, axis, a_len, b_len) = (sep.a, sep.b, sep.axis, sep.a_len as f64, sep.b_len as f64);
        let start_along = if matches!(axis, Axis::Vertical) { phys.x } else { phys.y };
        let vp = self.inner.viewports();
        let sum_weight = vp.root.find(a).map_or(1.0, |s| s.weight) + vp.root.find(b).map_or(1.0, |s| s.weight);
        self.inner.separator_drag = Some(SeparatorDrag { a, b, axis, start_along, a_len, b_len, sum_weight });
        true
    }

    fn update_separator_drag(&mut self, phys: smithay::utils::Point<f64, smithay::utils::Physical>) -> bool {
        use compositor_y5_viewport_state_base::state::Axis;
        let Some(d) = self.inner.separator_drag else { return false };
        let combined = d.a_len + d.b_len;
        if combined <= 1.0 {
            return true;
        }
        let along = if matches!(d.axis, Axis::Vertical) { phys.x } else { phys.y };
        let min = 40.0_f64.min(combined / 3.0);
        let new_a = (d.a_len + (along - d.start_along)).clamp(min, combined - min);
        let weight_a = d.sum_weight * (new_a / combined);
        let weight_b = d.sum_weight - weight_a;
        let vp = self.inner.viewports_mut();
        if let Some(s) = vp.root.find_mut(d.a) {
            s.weight = weight_a;
        }
        if let Some(s) = vp.root.find_mut(d.b) {
            s.weight = weight_b;
        }
        true
    }

    fn end_separator_drag(&mut self) {
        self.inner.separator_drag = None;
    }

    fn try_begin_floating_drag(&mut self, phys: smithay::utils::Point<f64, smithay::utils::Physical>, resize: bool) -> bool {
        use compositor_y5_viewport_state_base::state::Viewport;
        const M: f64 = 24.0;
        let (px, py) = (phys.x, phys.y);
        let found = self.inner.viewports().floating.iter().enumerate().rev().find_map(|(i, v)| {
            let Viewport::Floating { rect, .. } = v else { return None };
            let (x0, y0) = (rect.loc.x as f64, rect.loc.y as f64);
            let (x1, y1) = (x0 + rect.size.w as f64, y0 + rect.size.h as f64);
            if px < x0 - M || px > x1 + M || py < y0 - M || py > y1 + M {
                return None;
            }
            let (l, r) = ((px - x0).abs() <= M, (px - x1).abs() <= M);
            let (t, b) = ((py - y0).abs() <= M, (py - y1).abs() <= M);
            if l || r || t || b { Some((i, l, t, r, b, *rect)) } else { None }
        });
        let Some((index, left, top, right, bottom, start_rect)) = found else { return false };
        self.inner.floating_drag = Some(FloatingDrag { index, resize, left, top, right, bottom, start_cursor: (px, py), start_rect });
        true
    }

    fn update_floating_drag(&mut self, phys: smithay::utils::Point<f64, smithay::utils::Physical>) -> bool {
        use compositor_y5_viewport_state_base::state::Viewport;
        let Some(d) = self.inner.floating_drag else { return false };
        let (dx, dy) = (phys.x - d.start_cursor.0, phys.y - d.start_cursor.1);
        let vp = self.inner.viewports_mut();
        let Some(Viewport::Floating { rect, .. }) = vp.floating.get_mut(d.index) else { return true };
        const MIN: f64 = 100.0;
        if d.resize {
            let (mut x0, mut y0) = (d.start_rect.loc.x as f64, d.start_rect.loc.y as f64);
            let (mut x1, mut y1) = (x0 + d.start_rect.size.w as f64, y0 + d.start_rect.size.h as f64);
            if d.left { x0 = (x0 + dx).min(x1 - MIN); }
            if d.right { x1 = (x1 + dx).max(x0 + MIN); }
            if d.top { y0 = (y0 + dy).min(y1 - MIN); }
            if d.bottom { y1 = (y1 + dy).max(y0 + MIN); }
            *rect = smithay::utils::Rectangle::from_loc_and_size((x0.round() as i32, y0.round() as i32), ((x1 - x0).round() as i32, (y1 - y0).round() as i32));
        } else {
            *rect = smithay::utils::Rectangle::from_loc_and_size((d.start_rect.loc.x + dx.round() as i32, d.start_rect.loc.y + dy.round() as i32), d.start_rect.size);
        }
        true
    }

    fn end_floating_drag(&mut self) {
        self.inner.floating_drag = None;
    }
}
