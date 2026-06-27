//! Per-frame HUD pushes for the overview: the menu-bar clock (local HH:MM, pushed
//! once a minute) and the Display-panel FPS (EMA, pushed ~every 30 frames). The
//! throttles keep the iced surfaces from re-rendering every frame.
use std::cell::RefCell;
use std::time::Instant;
use compositor_orchestration_core_state_base::Loop;
use compositor_monitor_compositor_iced_base::IcedHandle;
use compositor_monitor_overview_ui_base::base::{OverviewMenu, OverviewMessage};
use compositor_configurator_settings_surface_message::message::SettingsMessage;
use compositor_configurator_settings_surface_view::Settings;
use compositor_orchestration_driver_settings_base::base::SETTINGS;

const DOW: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
const MON: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];

thread_local! {
    static CLOCK_MIN: RefCell<i64> = const { RefCell::new(-1) };
    static FPS: RefCell<(Option<Instant>, f32, u64)> = const { RefCell::new((None, 0.0, 0)) };
}

/// Called from the overview GLES prepare while the overlay is open.
pub fn per_frame(state: &mut Loop) {
    clock(state);
    fps(state);
}

fn now_tm() -> libc::tm {
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        tm
    }
}

fn clock(state: &mut Loop) {
    let Some(menu) = state.inner.overview().menu else { return };
    let tm = now_tm();
    let key = tm.tm_hour as i64 * 60 + tm.tm_min as i64;
    let changed = CLOCK_MIN.with(|c| { let mut c = c.borrow_mut(); if *c != key { *c = key; true } else { false } });
    if !changed {
        return;
    }
    let dow = DOW.get(tm.tm_wday as usize).copied().unwrap_or("");
    let mon = MON.get(tm.tm_mon as usize).copied().unwrap_or("");
    let label = format!("{dow} {:02} {mon}   ·   {:02}:{:02}", tm.tm_mday, tm.tm_hour, tm.tm_min);
    if let Some(reg) = state.inner.surface_mut().registry.as_mut() {
        let _ = reg.dispatch_message(IcedHandle::<OverviewMenu>::from_id(menu), OverviewMessage::Clock(label));
    }
}

fn fps(state: &mut Loop) {
    let now = Instant::now();
    let (ema, n) = FPS.with(|f| {
        let mut f = f.borrow_mut();
        if let Some(prev) = f.0 {
            let dt = now.duration_since(prev).as_secs_f32();
            if dt > 0.0 {
                let cur = 1.0 / dt;
                f.1 = if f.1 == 0.0 { cur } else { f.1 * 0.9 + cur * 0.1 };
            }
        }
        f.0 = Some(now);
        f.2 = f.2.wrapping_add(1);
        (f.1, f.2)
    });
    if n % 30 != 0 {
        return;
    }
    // Only push while the Performance tab is the visible module (gate set by the
    // forwarded Tab message) — other tabs shouldn't buffer per-frame FPS updates.
    let (wanted, handle) = {
        let st = state.inner.kernel.get(&SETTINGS);
        (st.fps_wanted, st.handle)
    };
    if !wanted {
        return;
    }
    let Some(handle) = handle else { return };
    if let Some(reg) = state.inner.surface_mut().registry.as_mut() {
        let _ = reg.dispatch_message(IcedHandle::<Settings>::from_id(handle), SettingsMessage::Fps(ema.round() as u32));
    }
}
