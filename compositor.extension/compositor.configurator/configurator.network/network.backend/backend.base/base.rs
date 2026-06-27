//! WiFi control via NetworkManager (system D-Bus, zbus blocking). A lazy
//! background worker polls NM ~every 1.5s into an `Arc<Mutex>` snapshot and
//! drains a command channel; the settings Network tab reads the snapshot and
//! sends commands. First access spawns the worker; it lives for the process.
use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::time::Duration;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{ObjectPath, OwnedObjectPath, Value};

const NM: &str = "org.freedesktop.NetworkManager";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WifiNet {
    pub ssid: String,
    pub strength: u8,
    pub secured: bool,
    pub active: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WifiSnapshot {
    pub available: bool,
    pub enabled: bool,
    pub networks: Vec<WifiNet>,
}

pub enum WifiCmd {
    SetEnabled(bool),
    Scan,
    Connect(String, String),
}

struct Handle {
    snap: Arc<Mutex<WifiSnapshot>>,
    tx: mpsc::Sender<WifiCmd>,
}
static HANDLE: OnceLock<Handle> = OnceLock::new();

fn handle() -> &'static Handle {
    HANDLE.get_or_init(|| {
        let snap = Arc::new(Mutex::new(WifiSnapshot::default()));
        let (tx, rx) = mpsc::channel();
        let s = snap.clone();
        std::thread::spawn(move || worker(s, rx));
        Handle { snap, tx }
    })
}

/// Current snapshot (cheap clone). Spawns the worker on first call.
pub fn snapshot() -> WifiSnapshot {
    handle().snap.lock().unwrap().clone()
}

/// Queue a command for the worker. Spawns the worker on first call.
pub fn command(cmd: WifiCmd) {
    let _ = handle().tx.send(cmd);
}

fn worker(snap: Arc<Mutex<WifiSnapshot>>, rx: mpsc::Receiver<WifiCmd>) {
    let Ok(conn) = Connection::system() else { return };
    loop {
        while let Ok(cmd) = rx.try_recv() {
            let _ = apply(&conn, cmd);
        }
        match poll(&conn) {
            Ok(s) => *snap.lock().unwrap() = s,
            Err(_) => snap.lock().unwrap().available = false,
        }
        std::thread::sleep(Duration::from_millis(1500));
    }
}

fn nm(conn: &Connection) -> zbus::Result<Proxy<'static>> {
    Proxy::new(conn, NM, "/org/freedesktop/NetworkManager", NM)
}

fn first_wifi(conn: &Connection) -> zbus::Result<Option<OwnedObjectPath>> {
    let devices: Vec<OwnedObjectPath> = nm(conn)?.call("GetAllDevices", &())?;
    for d in devices {
        let dev = Proxy::new(conn, NM, d.as_str().to_owned(), format!("{NM}.Device"))?;
        if dev.get_property::<u32>("DeviceType").unwrap_or(0) == 2 {
            return Ok(Some(d));
        }
    }
    Ok(None)
}

fn poll(conn: &Connection) -> zbus::Result<WifiSnapshot> {
    let enabled: bool = nm(conn)?.get_property("WirelessEnabled").unwrap_or(false);
    let mut by_ssid: HashMap<String, WifiNet> = HashMap::new();
    if let Some(d) = first_wifi(conn)? {
        let w = Proxy::new(conn, NM, d.as_str().to_owned(), format!("{NM}.Device.Wireless"))?;
        let active = w.get_property::<OwnedObjectPath>("ActiveAccessPoint").ok();
        let aps: Vec<OwnedObjectPath> = w.get_property("AccessPoints").unwrap_or_default();
        for ap in aps {
            let p = Proxy::new(conn, NM, ap.as_str().to_owned(), format!("{NM}.AccessPoint"))?;
            let ssid = String::from_utf8_lossy(&p.get_property::<Vec<u8>>("Ssid").unwrap_or_default()).into_owned();
            if ssid.is_empty() {
                continue;
            }
            let strength: u8 = p.get_property("Strength").unwrap_or(0);
            let rsn: u32 = p.get_property("RsnFlags").unwrap_or(0);
            let wpa: u32 = p.get_property("WpaFlags").unwrap_or(0);
            let entry = WifiNet {
                ssid: ssid.clone(),
                strength,
                secured: rsn != 0 || wpa != 0,
                active: active.as_ref() == Some(&ap),
            };
            by_ssid.entry(ssid).and_modify(|e| { if strength > e.strength { *e = entry.clone(); } }).or_insert(entry);
        }
    }
    let mut networks: Vec<WifiNet> = by_ssid.into_values().collect();
    networks.sort_by(|a, b| b.active.cmp(&a.active).then(b.strength.cmp(&a.strength)));
    Ok(WifiSnapshot { available: true, enabled, networks })
}

fn apply(conn: &Connection, cmd: WifiCmd) -> zbus::Result<()> {
    match cmd {
        WifiCmd::SetEnabled(on) => nm(conn)?.set_property("WirelessEnabled", on)?,
        WifiCmd::Scan => {
            if let Some(d) = first_wifi(conn)? {
                let w = Proxy::new(conn, NM, d.as_str().to_owned(), format!("{NM}.Device.Wireless"))?;
                let opts: HashMap<String, Value> = HashMap::new();
                let _: () = w.call("RequestScan", &(opts,))?;
            }
        }
        WifiCmd::Connect(ssid, psk) => {
            if let Some(dev) = first_wifi(conn)? {
                let mut con: HashMap<String, HashMap<String, Value>> = HashMap::new();
                con.insert("connection".into(), HashMap::from([
                    ("type".to_string(), Value::from("802-11-wireless".to_string())),
                    ("id".to_string(), Value::from(ssid.clone())),
                ]));
                con.insert("802-11-wireless".into(), HashMap::from([
                    ("ssid".to_string(), Value::from(ssid.as_bytes().to_vec())),
                ]));
                if !psk.is_empty() {
                    con.insert("802-11-wireless-security".into(), HashMap::from([
                        ("key-mgmt".to_string(), Value::from("wpa-psk".to_string())),
                        ("psk".to_string(), Value::from(psk)),
                    ]));
                }
                let root = ObjectPath::try_from("/").unwrap();
                let _: (OwnedObjectPath, OwnedObjectPath) =
                    nm(conn)?.call("AddAndActivateConnection", &(con, dev, root))?;
            }
        }
    }
    Ok(())
}
