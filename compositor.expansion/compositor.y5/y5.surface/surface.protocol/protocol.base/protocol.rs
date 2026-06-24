use compositor_y5_group_protocol_base::protocol::GroupBufferMessage;
use compositor_y5_graphic_capture_session::message::CaptureMessage;
use compositor_y5_launcher_protocol_message::message::LauncherMessage;
use compositor_y5_lock_interface_surface::message::LockMessage;
use compositor_y5_placeholder_protocol_base::message::PlaceholderMessage;
use compositor_y5_picker_surface_view::PickerSurfaceMessage;
use compositor_monitor_selection_scene_base::selection::{ScaleToFitOption, SelectionAction};

/// A selection-toolbar action forwarded from the iced UI's message handler to
/// the surface pump, where it is executed in-process against the canvas.
#[derive(Debug, Clone)]
pub enum SelectionForward {
    /// Apply the given align/distribute/stack actions (alt = repeat/start variant).
    Execute(Vec<SelectionAction>, bool),
    /// Scale the single selected window to fit the given aspect option.
    ScaleToFit(ScaleToFitOption),
}

#[derive(Debug)]
pub struct SurfaceMessage {
    // SurfaceID // <-- by HandlerID is available just in case its needed
    pub message: SurfaceMessageType
}

#[derive(Debug)]
pub enum SurfaceMessageType {
    Group(GroupBufferMessage),
    Placeholder(PlaceholderMessage),
    Launcher(LauncherMessage),
    LockScreen(LockMessage),
    Capture(CaptureMessage),
    Picker(PickerSurfaceMessage),
    Selection(SelectionForward),
}