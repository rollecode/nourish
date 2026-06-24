//! `StartTransientUnit` against `org.freedesktop.systemd1` (user bus).

use zbus::blocking::Connection;
use zbus::zvariant::Value;

/// Adopt `pid` into a transient scope named after `unit` (`.scope` appended if
/// absent). Best-effort: the process is already running, so a failure here only
/// means it misses cgroup isolation, not that the launch failed.
pub fn adopt_into_scope(pid: u32, unit: &str) -> Result<(), String> {
    let conn = Connection::session().map_err(|e| format!("session bus: {e}"))?;

    let name = if unit.ends_with(".scope") {
        unit.to_string()
    } else {
        format!("{unit}.scope")
    };

    // Manager.StartTransientUnit(name: s, mode: s, properties: a(sv), aux: a(sa(sv)))
    let properties: Vec<(&str, Value)> = vec![
        ("PIDs", Value::from(vec![pid])),
        ("Slice", Value::from("app.slice")),
        ("CollectMode", Value::from("inactive-or-failed")),
    ];
    let aux: Vec<(&str, Vec<(&str, Value)>)> = Vec::new();

    conn.call_method(
        Some("org.freedesktop.systemd1"),
        "/org/freedesktop/systemd1",
        Some("org.freedesktop.systemd1.Manager"),
        "StartTransientUnit",
        &(name.as_str(), "fail", properties, aux),
    )
    .map(|_| ())
    .map_err(|e| format!("StartTransientUnit({name}): {e}"))
}
