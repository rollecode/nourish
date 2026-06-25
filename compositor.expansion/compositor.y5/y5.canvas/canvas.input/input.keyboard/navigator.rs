use smithay::backend::input::KeyState;
use smithay::desktop::Window;
use smithay::input::keyboard::{KeysymHandle, ModifiersState};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::utils::{Logical, Point, Size};
use std::process::Command;
use uuid::Uuid;
use compositor_support_action_camera_find_base::find::Direction;
use compositor_support_action_camera_fit_aspect::aspect;
use compositor_y5_camera_zone_state::state::{Zone, ZoneSpecifier};
use compositor_orchestration_core_state_base::Loop;
use compositor_orchestration_core_state_base::state::{SetLockRequest, Status};
use compositor_y5_navigator_interface_base::interface::move_direction;
use compositor_y5_navigator_travel_state::state::{Target, Travel};
use compositor_support_library_input_keyboard_base::keyboard::combo::KeyCombo;
use compositor_support_library_input_keyboard_base::keyboard::handler::ShortcutHandler;
use compositor_support_library_input_keyboard_base::keyboard::key::Key;
use compositor_support_library_input_keyboard_base::shortcut;
use compositor_y5_window_interface_draw::fullscreen::fullscreen_unset_focused;
use compositor_y5_window_interface_record::window::LoopWindow;
use compositor_y5_window_lifecycle_interface::interface::TransformUpdate;

pub fn input_received(
    key_state: KeyState,
    modifiers: &ModifiersState,
    state: &mut Loop,
    key: Option<Key>,
) -> Option<bool> {
    if key.is_none() {
        return Some(true);
    }

    let is_press = key_state == KeyState::Pressed;
    // Only handle presses.
    if !is_press {
        return Some(true);
    }

    // Build the vector manually using standard Rust struct initialization.
    // We only use the single `shortcut!` macro to generate the KeyCombo.
    let handlers: Vec<ShortcutHandler<Loop>> = inline_shortcut_handlers(state.inner.storage.nested);

    // Iterate through the vector (Top to Bottom priority)
    for handler in handlers {
        if handler.combo.matches(modifiers, key) {
            if (handler.action)(state) {
                return None;
            }
        }
    }

    Some(true)
}

fn launcher_delegate(p0: &mut Loop) {
    compositor_y5_launcher_interface_base::interface::start_defered(p0)
}

fn debug_delegate(state: &mut Loop) {
    // let space = &state.inner.space_state().state;
    // let dh = &state.state.output.display_handle;
    // let registry = default_registry();
    // let meta: Vec<_> = state
    //     .inner
    //     .canvas
    //     .Select
    //     .Selection
    //     .clone()
    //     .iter()
    //     .filter_map(|w| {
    //         let meta =
    //             y5_compositor_introspection_extraction::meta::wayland::extract_node_from_window(
    //                 w, space, dh,
    //             );
    //         if meta.is_none() {
    //             return None;
    //         }
    //         let meta = meta.unwrap();
    //
    //         // Just see what we know:
    //         let hints = extract(&meta, None, &registry);
    //         for raw in hints.iter_raw() {
    //             println!("{}: {:?} ({:?})", raw.attr_name, raw.source, raw.confidence);
    //         }
    //
    //         // Full pin:
    //         let result = pin(meta, None, &registry);
    //
    //         if result.is_err() {
    //             return None;
    //         }
    //
    //         let result = result.unwrap();
    //         let (record, draft) = result;
    //         // y5_compositor_introspection_extraction::plan::placeholder::pin_window(meta_tree, &hr, None),
    //
    //         Some((record, draft))
    //     })
    //     .collect();
    //
    // // Extract the meta with new crates
    // // get active windows
    // println!("{:?}", meta);
}

fn zone_delegate(state: &mut Loop, zone: &str, register: bool) {
    if register {
        // CHECK: From given selection, it must be availabvle windows only. doesnt matter since they wont participate next step.
        // they may become "hidden". this needs to be handled on window close ( whenever space deletes that window )
        let selected: Vec<Uuid> = state.inner.select()
            
            .Selection
            .clone()
            .iter()
            .filter_map(|w| w.uuid())
            .collect();

        if selected.is_empty() {
            let position = state.inner.camera_mut().transform.position();
            let position = (position.x, position.y);
            let zoom = *state.inner.camera_mut().transform.zoom();

            state.inner.camera_mut().zone.zone.insert(
                zone.into(),
                Zone {
                    specifier: ZoneSpecifier::Camera {
                        zoom: zoom,
                        position: position,
                    },
                },
            );
        } else {
            state.inner.camera_mut().zone.zone.insert(
                zone.into(),
                Zone {
                    specifier: ZoneSpecifier::Element { UUID: selected },
                },
            );
        }
    } else {
        let zone = String::from(zone);
        let zone = state.inner.camera().zone.zone.get(&zone);
        if zone.is_none() {
            return;
        }
        let zone = zone.unwrap();

        match &zone.specifier {
            ZoneSpecifier::Element { UUID } => {
                let windows: Vec<Window> = state
                    .inner.space_state()
                    .state
                    .elements()
                    .filter_map(|f| {
                        let f = f.clone();
                        let uuid = f.uuid();
                        if uuid.is_none() {
                            return None;
                        }
                        let uuid = uuid.unwrap();

                        if !UUID.contains(&uuid) {
                            return None;
                        }

                        return Some(f);
                    })
                    .collect();

                let windows = windows.iter().map(|w| w).collect();
                let result =
                    compositor_y5_navigator_travel_machine::view::view(state, windows, false);
                let zoom = result.zoom.and_then(|target| {
                    return Some(Target {
                        start: None,
                        target,
                    });
                });

                let position = result.position.and_then(|target| {
                    return Some(Target {
                        start: None,
                        target: result.position.unwrap(),
                    });
                });

                let travel = Travel {
                    position: position,
                    zoom: zoom,
                    duration: None,
                    time_start: None,
                };

                state.inner.navigator_mut().set(
                    compositor_y5_navigator_state_base::state::State::Travel(travel),
                );
            }
            ZoneSpecifier::Camera { position, zoom } => {
                let zoom = Some(Target {
                    start: None,
                    target: zoom.clone(),
                });

                let position = Some(Target {
                    start: None,
                    target: position.clone(),
                });

                let travel = Travel {
                    position: position,
                    zoom: zoom,
                    duration: None,
                    time_start: None,
                };

                state.inner.navigator_mut().set(
                    compositor_y5_navigator_state_base::state::State::Travel(travel),
                );
            }
        }
    }
}

fn group_delegate(state: &mut Loop, join: bool,) {
    if join {
        compositor_y5_group_interface_base::interface::selection_set_join(state);
        
    } else {
    
        compositor_y5_group_interface_base::interface::selection_set(state);
    }
}

fn zoom_delegate(state: &mut Loop, zoom_1: bool, fit_1: bool) {
    compositor_y5_navigator_interface_base::interface::fit(state, zoom_1, fit_1);
}

// // // Only locks. results in showing GDM
// fn lock(s: &mut Loop) {
//     // if s.is_locked() {
//     //     return;
//     // }
//
//     // This is what actually locks the screen. The client connects to your
//     // ext-session-lock-v1 implementation, your SessionLockHandler::lock fires,
//     // you blank outputs, present a frame, call SessionLocker::lock(), done.
//     let _ = Command::new("gtklock") // or "swaylock", "hyprlock"
//         .spawn();
//
//     // Tell the rest of the system we're locked, for tools like
//     // gnome-keyring that watch this flag. Optional but polite.
//     let _ = Command::new("loginctl")
//         .args(["lock-session"])
//         .status();
//
//     // // Avoid double-locking.
//     // if s.is_locked() {
//     //     return;
//     // }
//     //
//     //
//     //
//     // // Tell logind the session is logically locked. lock clients and
//     // // other listeners (e.g. gnome-keyring) observe this via PropertiesChanged.
//     // if let Ok(conn) = Connection::system() {
//     //     let _ = conn.call_method(
//     //         Some("org.freedesktop.login1"),
//     //         "/org/freedesktop/login1/session/auto",
//     //         Some("org.freedesktop.login1.Session"),
//     //         "SetLockedHint",
//     //         &(true,),
//     //     );
//     // }
//     //
//     //
//     // let _ = Command::new("loginctl")
//     //     .args(["lock-session"]) // operates on the caller's session by default
//     //     .status();
//     //
//     // Spawn whichever ext-session-lock-v1 client you want as the unlock UI.
//     // // gtklock / hyprlock / swaylock all work. Configure this; don't hardcode.
//     // let _ = Command::new(&s.config.lock_command) // e.g. "gtklock"
//     //     .spawn();
//
//     // From here on, your SessionLockHandler::lock() impl will fire when
//     // the client binds ext_session_lock_manager_v1 and calls lock(). That
//     // handler is where you call SessionLocker::lock() after presenting a
//     // blanked frame on every output.
//
// }
//
// // fn lock(s: &mut Loop) {
// //     if s.is_locked() {
// //         return;
// //     }
// //
// //     // This is what actually locks the screen. The client connects to your
// //     // ext-session-lock-v1 implementation, your SessionLockHandler::lock fires,
// //     // you blank outputs, present a frame, call SessionLocker::lock(), done.
// //     let _ = Command::new("gtklock") // or "swaylock", "hyprlock"
// //         .spawn();
// //
// //     // Tell the rest of the system we're locked, for tools like
// //     // gnome-keyring that watch this flag. Optional but polite.
// //     let _ = Command::new("loginctl")
// //         .args(["lock-session"])
// //         .status();
// // }
//
// //
//
//
// // // Executes both lock and sleep.
// fn sleep(s: &mut Loop) {
//     // There is an "inhibitor" way to way for lock which is better.
//     lock(s);
//     // s.wait_until_locked(Duration::from_millis(500));
//
//     let _ = Command::new("systemctl")
//         .arg("suspend")
//         .spawn(); // fire and forget; suspend doesn't return until resume
// }
//
// // Completely terminate the session and all process within it.
// // It needs to be like regular gnome logout.
fn lock(s: &mut Loop) {
    // CHECK: right now defers by setting this flag, but should insert a source or have access to gles renderer here
    s.inner.__set_lock = Some(SetLockRequest { sleep: false });
}

fn terminate(s: &mut Loop) {
    // Stop the target. --no-block returns immediately; without it,
    // systemctl waits for the job to complete (which is what you probably want).
    let _ = Command::new("systemctl")
        .args(["--user", "restart", "graphical-session.target"])
        .status(); // blocks until systemctl returns

    s.inner.loader.loop_signal.stop();
    s.inner.status = Status::Terminate;
}

pub fn inline_shortcut_handlers(nosuper: bool) -> Vec<ShortcutHandler<Loop>> {
    let mut handlers: Vec<ShortcutHandler<Loop>> = vec![
        ShortcutHandler {
            combo: shortcut!(Super + Alt + L), // <-- Terminates everything.
            action: Box::new(move |s| {
                terminate(s);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Super + N),
            action: Box::new(move |s| {
                launcher_delegate(s);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Super + S),
            action: Box::new(move |s| {
                // Open the capture setup overlay (consumes the current canvas
                // window selection as the default target).
                compositor_y5_graphic_capture_interface::interface::request_setup(s);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(F10),
            action: Box::new(move |s| {
                debug_delegate(s);

                true
            }),
        },
        ShortcutHandler {
            // F11: only exits fullscreen on the keyboard-focused window (if a
            // client put it there via the protocol). Returns false when there
            // is nothing to exit, so the key falls through to the client.
            combo: shortcut!(F11),
            action: Box::new(move |s| fullscreen_unset_focused(s)),
        },
        ShortcutHandler {
            combo: shortcut!(F1),
            action: Box::new(move |s| {
                zone_delegate(s, "f1", false);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(F2),
            action: Box::new(move |s| {
                zone_delegate(s, "f2", false);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(F3),
            action: Box::new(move |s| {
                zone_delegate(s, "f3", false);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(F4),
            action: Box::new(move |s| {
                zone_delegate(s, "f4", false);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(F5),
            action: Box::new(move |s| {
                zone_delegate(s, "f5", false);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(F6),
            action: Box::new(move |s| {
                zone_delegate(s, "f6", false);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Shift + F1),
            action: Box::new(move |s| {
                zone_delegate(s, "f1", true);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Shift + F2),
            action: Box::new(move |s| {
                zone_delegate(s, "f2", true);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Shift + F3),
            action: Box::new(move |s| {
                zone_delegate(s, "f3", true);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Shift + F4),
            action: Box::new(move |s| {
                zone_delegate(s, "f4", true);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Shift + F5),
            action: Box::new(move |s| {
                zone_delegate(s, "f5", true);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Shift + F6),
            action: Box::new(move |s| {
                zone_delegate(s, "f6", true);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Super + Alt + G),
            action: Box::new(move |s| {
                group_delegate(s, true);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Super + G),
            action: Box::new(move |s| {
                group_delegate(s, false);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Super + Alt + F),
            action: Box::new(move |s| {
                zoom_delegate(s, false, false);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Super + F),
            action: Box::new(move |s| {
                zoom_delegate(s, true, false);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Super + Shift + Alt + F),
            action: Box::new(move |s| {
                zoom_delegate(s, true, true);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Super + Right),
            action: Box::new(move |s| {
                move_direction(s, Direction::Right, true);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Super + Left),
            action: Box::new(move |s| {
                move_direction(s, Direction::Left, true);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Super + Up),
            action: Box::new(move |s| {
                move_direction(s, Direction::Up, true);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Super + Down),
            action: Box::new(move |s| {
                move_direction(s, Direction::Down, true);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Super + Alt + Right),
            action: Box::new(move |s| {
                move_direction(s, Direction::Right, false);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Super + Alt + Left),
            action: Box::new(move |s| {
                move_direction(s, Direction::Left, false);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Super + Alt + Up),
            action: Box::new(move |s| {
                move_direction(s, Direction::Up, false);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Super + Alt + Down), // <-- should be alt but intellij seems to intercept(or something else)
            action: Box::new(move |s| {
                move_direction(s, Direction::Down, false);
                true
            }),
        },
        ShortcutHandler {
            combo: shortcut!(Super + L),
            action: Box::new(move |s| {
                lock(s);
                true
            }),
        },
    ];

    if nosuper {
        for w in &mut handlers {
            if w.combo.modifiers.logo {
                w.combo.modifiers.logo = false;
                w.combo.modifiers.ctrl = true;
            }
        }
    }

    handlers
}
