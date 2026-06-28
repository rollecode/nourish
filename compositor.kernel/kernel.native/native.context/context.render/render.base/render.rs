//! The per-pipe render context (moved from udev.draw/draw.context). Owns OUR
//! names for the hosted pipe objects via `scanout.surface/surface.output`,
//! the tap subscriptions the frame executor consults (Law 5: taps fire only
//! for active subscribers), and the live Law-7 safety-net enablement set
//! (seeded from preference, updated through `device.interface`).

use compositor_kernel_scanout_surface_output_base::output::{
    NativeDrmOutput, NativeDrmOutputManager,
};
use compositor_kernel_graphic_draw_plan_tap::tap::TapSubscriptions;
use compositor_kernel_graphic_preference_enable_safety::safety::SafetyEnable;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::output::{Mode, Output};
use smithay::reexports::calloop::RegistrationToken;
use smithay::reexports::drm::control::Mode as DrmMode;
use smithay::reexports::input::Libinput;
use smithay::reexports::wayland_server::DisplayHandle;
use std::cell::RefCell;
use std::rc::Rc;
use compositor_orchestration_core_state_base::state::StateDRMBinding;

pub struct NativeRenderContext {
    pub display_handle: DisplayHandle,
    pub mode: Mode,
    pub output: Output,
    pub damage_tracker: OutputDamageTracker,
    pub drm_output: NativeDrmOutput,
    pub drm_output_manager: Rc<RefCell<NativeDrmOutputManager>>,
    pub gpu_binding: Rc<RefCell<StateDRMBinding>>,
    pub libinput_context: Libinput,
    pub tap_subscriptions: TapSubscriptions,
    /// COMPOSITOR_RENDERER=vulkan: compose the scene with the VulkanRenderer and
    /// scan it out via the same DrmOutput (the GLES multigpu is still used for
    /// the per-frame iced/bevy/parallax GLES `prepare()`). Default false (GLES).
    pub vulkan_mode: bool,
    /// The native VulkanRenderer (built at wire time when vulkan_mode), bound to
    /// the primary render node.
    pub vulkan: Option<compositor_kernel_vulkan_renderer_core_base::renderer::VulkanRenderer>,
    /// Law-7 enablement, live: seeded from
    /// `compositor_kernel_graphic_preference_enable_safety::safety::get()` at wiring,
    /// runtime-updated through `device.interface` (the integration surface).
    pub safety: SafetyEnable,
    /// HDR (M5): the display's parsed EDID HDR/colorimetry caps.
    pub hdr_caps: compositor_kernel_drm_edid_parse_base::parse::HdrInfo,
    /// HDR output path active this session (`COMPOSITOR_HDR` + capable display +
    /// Vulkan). When true the executor signals the connector (BT.2020 + PQ) once
    /// and composites in the HDR working space.
    pub hdr_active: bool,
    /// Whether the one-time DRM HDR output signalling has been applied.
    pub hdr_signalled: bool,
    /// Raw DRM fd + connector handle for the one-time HDR property commit
    /// (smithay's DrmCompositor doesn't expose colorspace / HDR metadata).
    pub drm_fd: smithay::backend::drm::DrmDeviceFd,
    pub connector: smithay::reexports::drm::control::connector::Handle,
    /// The mode currently driving the pipe. Seeded at wire time and updated on
    /// every successful live mode change (`native.context/context.display/
    /// display.mode`) — the baseline an auto-revert restores to.
    pub current_drm_mode: DrmMode,
    /// The connector's advertised modes (from EDID), so the live mode-change
    /// drain can resolve a requested width/height/refresh to a `DrmMode` without
    /// re-probing the connector.
    pub modes: Vec<DrmMode>,
    /// Armed confirm/revert watchdog for a provisionally-applied mode:
    /// `(previous_mode, one_shot_timer)`. `Some` while awaiting the user's Keep;
    /// cleared on Confirm, on Revert, or when the timer reverts. See `display.mode`.
    pub mode_revert: Option<(DrmMode, RegistrationToken)>,
}
