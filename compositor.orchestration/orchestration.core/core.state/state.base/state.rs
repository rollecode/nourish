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
pub struct Orchestrator {
    pub start_time: std::time::Instant,
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
        self.worlds.get(target).storage().get(&compositor_y5_camera_state_base::state::CAMERA)
    }

    pub fn camera_mut(&mut self) -> &mut compositor_y5_camera_state_base::state::Camera {
        let target = self.worlds.spawn_target();
        self.worlds.get_mut(target).storage_mut().get_mut(&compositor_y5_camera_state_base::state::CAMERA_MUT)
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
    fn size_context(&self) -> compositor_y5_camera_transform_translate::transform::Context;
}
impl CoordinateTrait for Loop {
    fn size_context(&self) -> compositor_y5_camera_transform_translate::transform::Context {
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

        // let physical = self.inner.space_state().default_physical_precise();
        //
        // compositor_y5_camera_transform_translate::transform::Context::new(
        //     (
        //         self.inner.camera_mut().transform.position.x,
        //         self.inner.camera_mut().transform.position.y,
        //     ),
        //     self.inner.camera_mut().transform.zoom,
        //     (physical.size.w, physical.size.h),
        //     self.inner.space_state().default_scale().fractional_scale(),
        // )
    }
}
