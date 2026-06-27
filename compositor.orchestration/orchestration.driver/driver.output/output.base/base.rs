//! Output-mode driver channel between the rim (settings window, holds `&mut Loop`)
//! and the kernel (owns the live `NativeDrmOutput` inside its calloop sources).
//! The rim cannot touch the DRM output directly, so — like the lid driver — it
//! writes a primitive request the kernel loop drains, and reads back a primitive
//! snapshot/result the kernel writes. All values are primitive: no smithay/DRM
//! types cross the layer boundary.

use compositor_support_system_storage_token_base::base::{Token, TokenMut};

/// One advertised mode, primitive form (refresh in mHz to match DRM `vrefresh*1000`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeInfo {
    pub width: u16,
    pub height: u16,
    pub refresh_mhz: u32,
}

/// Rim → kernel: a step in the user-confirmed mode-change transaction.
/// `Apply` provisionally switches and arms the confirm/revert watchdog; `Confirm`
/// (user kept it) makes it permanent; `Revert` (user declined / dialog closed)
/// restores the previous mode now. The kernel drains at most one per loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputModeRequest {
    Apply { width: u16, height: u16, refresh_mhz: u32 },
    Confirm,
    Revert,
}

/// Kernel → rim: the connector's advertised modes for the settings UI to list,
/// plus its current mode and a stable EDID identity ("make model serial").
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutputModesSnapshot {
    pub edid_key: String,
    pub current: Option<ModeInfo>,
    pub available: Vec<ModeInfo>,
}

/// Kernel → rim: outcome of the last `Apply`, driving the UI confirmation dialog.
/// `Provisional` = applied, awaiting Keep/Revert (show countdown); `Confirmed` =
/// kept (UI then persists to preferences.json); `Reverted` = restored (user
/// declined, dialog timed out, or no signal); `Failed` = could not apply at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyResult {
    Provisional,
    Confirmed,
    Reverted,
    Failed,
}

/// Rim-issued mode request, drained by the kernel loop (`None` when idle).
pub static OUTPUT_MODE_REQUEST: Token<Option<OutputModeRequest>> = Token::new();
pub static OUTPUT_MODE_REQUEST_MUT: TokenMut<Option<OutputModeRequest>> =
    TokenMut::new(&OUTPUT_MODE_REQUEST);

/// Kernel-written advertised-mode snapshot, read by the settings UI.
pub static OUTPUT_MODES_SNAPSHOT: Token<OutputModesSnapshot> = Token::new();
pub static OUTPUT_MODES_SNAPSHOT_MUT: TokenMut<OutputModesSnapshot> =
    TokenMut::new(&OUTPUT_MODES_SNAPSHOT);

/// Kernel-written result of the last apply (`None` until the first transaction).
pub static OUTPUT_MODE_RESULT: Token<Option<ApplyResult>> = Token::new();
pub static OUTPUT_MODE_RESULT_MUT: TokenMut<Option<ApplyResult>> =
    TokenMut::new(&OUTPUT_MODE_RESULT);
