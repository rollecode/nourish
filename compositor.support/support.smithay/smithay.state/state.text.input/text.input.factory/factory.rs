use smithay::{
    input::SeatHandler,
    reexports::{
        wayland_protocols::wp::text_input::zv3::server::{
            zwp_text_input_manager_v3::ZwpTextInputManagerV3, zwp_text_input_v3::ZwpTextInputV3,
        },
        wayland_protocols_misc::zwp_input_method_v2::server::{
            zwp_input_method_manager_v2::ZwpInputMethodManagerV2,
            zwp_input_method_v2::ZwpInputMethodV2,
        },
        wayland_protocols_misc::zwp_virtual_keyboard_v1::server::{
            zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
            zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
        },
        wayland_server::{Dispatch, DisplayHandle, GlobalDispatch},
    },
    wayland::{
        GlobalData,
        input_method::{
            InputMethodManagerGlobalData, InputMethodManagerState, InputMethodUserData,
        },
        text_input::{TextInputManagerState, TextInputUserData},
        virtual_keyboard::{
            VirtualKeyboardManagerGlobalData, VirtualKeyboardManagerState, VirtualKeyboardUserData,
        },
    },
};
use compositor_support_smithay_dispatch_state_base::state::DispatchWire;
use compositor_support_smithay_state_text_input_base::state::TextInput;
use compositor_support_smithay_state_text_input_launch::launch::is_authorized;

pub fn new<I: DispatchWire>(display_handle: &DisplayHandle) -> TextInput
where
    I: GlobalDispatch<ZwpTextInputManagerV3, GlobalData>,
    I: Dispatch<ZwpTextInputManagerV3, GlobalData>,
    I: Dispatch<ZwpTextInputV3, TextInputUserData>,
    I: 'static,
    I: GlobalDispatch<ZwpInputMethodManagerV2, InputMethodManagerGlobalData>,
    I: Dispatch<ZwpInputMethodManagerV2, GlobalData>,
    I: Dispatch<ZwpInputMethodV2, InputMethodUserData<I>>,
    I: GlobalDispatch<ZwpVirtualKeyboardManagerV1, VirtualKeyboardManagerGlobalData>,
    I: Dispatch<ZwpVirtualKeyboardManagerV1, GlobalData>,
    I: Dispatch<ZwpVirtualKeyboardV1, VirtualKeyboardUserData<I>>,
    I: SeatHandler,
    I: 'static,
{
    // `can_view` gate for both managers: these grant system-wide input power (keyboard grab ==
    // keylogger; key/text injection), so ONLY the compositor-launched IME process group may bind
    // them (see `text.input.launch`). Same predicate for the virtual keyboard — it injects keys
    // into the focused app, so it is exactly as sensitive.
    let ime_dh = display_handle.clone();
    let input_method_manager_state =
        InputMethodManagerState::new::<I, _>(&display_handle, move |client| {
            is_authorized(client, &ime_dh)
        });
    let text_input_manager_state = TextInputManagerState::new::<I>(&display_handle);

    let vk_dh = display_handle.clone();
    let virtual_keyboard_manager_state =
        VirtualKeyboardManagerState::new::<I, _>(&display_handle, move |client| {
            is_authorized(client, &vk_dh)
        });

    TextInput {
        input_method_manager_state,
        text_input_manager_state,
        virtual_keyboard_manager_state,
    }
}
