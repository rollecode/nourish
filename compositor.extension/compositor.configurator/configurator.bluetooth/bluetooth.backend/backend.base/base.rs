//! Bluetooth control via BlueZ (system D-Bus, zbus blocking). A lazy background
//! worker polls BlueZ ~every 1.5s into an `Arc<Mutex>` snapshot and drains a
//! command channel; the settings Bluetooth tab reads the snapshot + sends
//! commands. Scan is started/stopped by the tab's visibility (see the interface).
use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::time::Duration;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

const BLUEZ: &str = "org.bluez";
type Managed = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BtDev {
    pub path: String,
    pub address: String,
    pub name: String,
    pub paired: bool,
    pub connected: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BtSnapshot {
    pub available: bool,
    pub powered: bool,
    pub discovering: bool,
    pub devices: Vec<BtDev>,
}

pub enum BtCmd {
    SetPowered(bool),
    Scan(bool),
    Pair(String),
    Connect(String),
}

struct Handle {
    snap: Arc<Mutex<BtSnapshot>>,
    tx: mpsc::Sender<BtCmd>,
}
static HANDLE: OnceLock<Handle> = OnceLock::new();

fn handle() -> &'static Handle {
    HANDLE.get_or_init(|| {
        let snap = Arc::new(Mutex::new(BtSnapshot::default()));
        let (tx, rx) = mpsc::channel();
        let s = snap.clone();
        std::thread::spawn(move || worker(s, rx));
        Handle { snap, tx }
    })
}

pub fn snapshot() -> BtSnapshot {
    handle().snap.lock().unwrap().clone()
}

pub fn command(cmd: BtCmd) {
    let _ = handle().tx.send(cmd);
}

fn worker(snap: Arc<Mutex<BtSnapshot>>, rx: mpsc::Receiver<BtCmd>) {
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

fn managed(conn: &Connection) -> zbus::Result<Managed> {
    Proxy::new(conn, BLUEZ, "/", "org.freedesktop.DBus.ObjectManager")?.call("GetManagedObjects", &())
}

fn poll(conn: &Connection) -> zbus::Result<BtSnapshot> {
    let objs = managed(conn)?;
    let (mut powered, mut discovering) = (false, false);
    let mut devices = Vec::new();
    for (path, ifaces) in &objs {
        if let Some(a) = ifaces.get("org.bluez.Adapter1") {
            powered = prop_bool(a, "Powered");
            discovering = prop_bool(a, "Discovering");
        }
        if let Some(d) = ifaces.get("org.bluez.Device1") {
            devices.push(BtDev {
                path: path.as_str().to_string(),
                address: prop_str(d, "Address"),
                name: prop_str(d, "Name"),
                paired: prop_bool(d, "Paired"),
                connected: prop_bool(d, "Connected"),
            });
        }
    }
    devices.sort_by(|a, b| b.connected.cmp(&a.connected).then(b.paired.cmp(&a.paired)).then(a.name.cmp(&b.name)));
    Ok(BtSnapshot { available: true, powered, discovering, devices })
}

fn prop_bool(m: &HashMap<String, OwnedValue>, k: &str) -> bool {
    m.get(k).and_then(|v| bool::try_from(v.clone()).ok()).unwrap_or(false)
}
fn prop_str(m: &HashMap<String, OwnedValue>, k: &str) -> String {
    m.get(k).and_then(|v| String::try_from(v.clone()).ok()).unwrap_or_default()
}

fn adapter(conn: &Connection) -> zbus::Result<Option<OwnedObjectPath>> {
    Ok(managed(conn)?.into_iter().find(|(_, i)| i.contains_key("org.bluez.Adapter1")).map(|(p, _)| p))
}

fn apply(conn: &Connection, cmd: BtCmd) -> zbus::Result<()> {
    match cmd {
        BtCmd::SetPowered(on) => {
            if let Some(a) = adapter(conn)? {
                Proxy::new(conn, BLUEZ, a.as_str().to_owned(), "org.bluez.Adapter1")?.set_property("Powered", on)?;
            }
        }
        BtCmd::Scan(on) => {
            if let Some(a) = adapter(conn)? {
                let ad = Proxy::new(conn, BLUEZ, a.as_str().to_owned(), "org.bluez.Adapter1")?;
                let _: () = ad.call(if on { "StartDiscovery" } else { "StopDiscovery" }, &())?;
            }
        }
        BtCmd::Pair(path) => {
            let _: () = Proxy::new(conn, BLUEZ, path, "org.bluez.Device1")?.call("Pair", &())?;
        }
        BtCmd::Connect(path) => {
            let _: () = Proxy::new(conn, BLUEZ, path, "org.bluez.Device1")?.call("Connect", &())?;
        }
    }
    Ok(())
}
