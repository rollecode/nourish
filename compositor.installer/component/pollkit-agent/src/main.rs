use iced::widget::{button, column, row, text, text_input};
use iced::{Alignment, Element, Task};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::{Mutex, oneshot};
use zbus::{connection, interface, proxy};

/// Errors surfaced to polkit. The `org.freedesktop.PolicyKit1.Error.Cancelled`
/// name is meaningful — polkit uses it to tell "user dismissed the dialog" apart
/// from "authentication failed", so it stops re-prompting instead of showing an
/// error. Everything else maps to `.Failed`.
#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.PolicyKit1.Error")]
enum AgentError {
    #[zbus(error)]
    ZBus(zbus::Error),
    Failed(String),
    Cancelled(String),
}

// ---- D-Bus Proxy Definitions ----

#[proxy(
    interface = "org.freedesktop.PolicyKit1.Authority",
    default_service = "org.freedesktop.PolicyKit1",
    default_path = "/org/freedesktop/PolicyKit1/Authority"
)]
trait Authority {
    fn register_authentication_agent(
        &self,
        subject: &Subject,
        locale: &str,
        object_path: &str,
    ) -> zbus::Result<()>;

    fn unregister_authentication_agent(
        &self,
        subject: &Subject,
        object_path: &str,
    ) -> zbus::Result<()>;
}

#[derive(serde::Serialize, serde::Deserialize, zbus::zvariant::Type)]
struct Subject {
    subject_kind: String,
    subject_details: HashMap<String, zbus::zvariant::OwnedValue>,
}

// ---- logind (login1) Proxies for session discovery ----

#[proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait LoginManager {
    /// Returns (session_id, uid, user_name, seat_id, object_path) per session.
    fn list_sessions(
        &self,
    ) -> zbus::Result<Vec<(String, u32, String, String, zbus::zvariant::OwnedObjectPath)>>;

    fn get_user(&self, uid: u32) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
}

#[proxy(
    interface = "org.freedesktop.login1.User",
    default_service = "org.freedesktop.login1"
)]
trait LoginUser {
    /// The user's "display" (graphical) session: (session_id, object_path).
    #[zbus(property)]
    fn display(&self) -> zbus::Result<(String, zbus::zvariant::OwnedObjectPath)>;
}

#[proxy(
    interface = "org.freedesktop.login1.Session",
    default_service = "org.freedesktop.login1"
)]
trait LoginSession {
    #[zbus(property)]
    fn active(&self) -> zbus::Result<bool>;

    #[zbus(property, name = "Type")]
    fn type_(&self) -> zbus::Result<String>;
}

// ---- Identity selection ----

const HELPER: &str = "/usr/lib/polkit-1/polkit-agent-helper-1";

/// All group IDs the current user belongs to (primary + supplementary), via `id -G`.
fn current_gids() -> Vec<u32> {
    std::process::Command::new("id")
        .arg("-G")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Resolve a uid to a login name (`id -nu <uid>`).
fn uid_to_name(uid: u32) -> Option<String> {
    let out = std::process::Command::new("id")
        .arg("-nu")
        .arg(uid.to_string())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!name.is_empty()).then_some(name)
}

/// Pick which identity to authenticate as, from polkit's list of *allowed*
/// identities for the action. This is the crux of `auth_admin` actions working:
///
///   * `unix-user` identities carry a `uid`; `unix-group` identities carry a `gid`.
///   * Prefer **self** — if the current user is directly listed, or is a member of
///     any allowed group (e.g. `wheel`) — so the user types their OWN password.
///     This is what makes Fedora Media Writer (auth_admin → wheel/root) work: as a
///     wheel member you authenticate as yourself instead of as root.
///   * Otherwise prefer a non-root user, then root, then whatever is first. Falling
///     back to `root` last matters because root is password-locked on many systems,
///     so it can never authenticate.
fn choose_target_user(identities: &[Subject]) -> Option<String> {
    let my_uid = current_uid();
    let my_gids = current_gids();

    let mut user_uids: Vec<u32> = Vec::new();
    let mut self_eligible = false;

    for id in identities {
        match id.subject_kind.as_str() {
            "unix-user" => {
                if let Some(uid) = id.subject_details.get("uid").and_then(|v| u32::try_from(v).ok())
                {
                    user_uids.push(uid);
                    if Some(uid) == my_uid {
                        self_eligible = true;
                    }
                }
            }
            "unix-group" => {
                if let Some(gid) = id.subject_details.get("gid").and_then(|v| u32::try_from(v).ok())
                {
                    if my_gids.contains(&gid) {
                        self_eligible = true;
                    }
                }
            }
            _ => {}
        }
    }

    // 1. Authenticate as ourselves whenever we're allowed to.
    if self_eligible {
        if let Some(uid) = my_uid {
            if let Some(name) = uid_to_name(uid) {
                return Some(name);
            }
        }
    }
    // 2. A named non-root admin.
    if let Some(&uid) = user_uids.iter().find(|&&u| u != 0) {
        if let Some(name) = uid_to_name(uid) {
            return Some(name);
        }
    }
    // 3. root, then 4. anything at all.
    if user_uids.contains(&0) {
        return Some("root".to_string());
    }
    user_uids.first().and_then(|&uid| uid_to_name(uid))
}

// ---- PAM conversation via polkit-agent-helper-1 ----

/// Drive the PAM conversation for `target_user` with the collected `secret`.
/// Returns `Ok(true)` on `SUCCESS`, `Ok(false)` on `FAILURE`, `Err` on I/O trouble.
async fn run_pam(target_user: &str, cookie: &str, secret: &str) -> Result<bool, AgentError> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let mut helper = tokio::process::Command::new(HELPER)
        .arg(target_user)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true) // so a cancelled request tears the helper down
        .spawn()
        .map_err(|e| AgentError::Failed(format!("spawn helper: {e}")))?;

    let mut helper_in = helper
        .stdin
        .take()
        .ok_or_else(|| AgentError::Failed("helper stdin missing".into()))?;
    let helper_out = helper
        .stdout
        .take()
        .ok_or_else(|| AgentError::Failed("helper stdout missing".into()))?;

    // Protocol: the FIRST stdin line is the cookie, then a line-based PAM
    // conversation follows, terminated by SUCCESS/FAILURE on stdout.
    helper_in
        .write_all(format!("{cookie}\n").as_bytes())
        .await
        .map_err(|e| AgentError::Failed(format!("write cookie: {e}")))?;
    helper_in.flush().await.ok();

    let mut verdict = false;
    let mut lines = BufReader::new(helper_out).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if line.starts_with("PAM_PROMPT_ECHO_OFF") || line.starts_with("PAM_PROMPT_ECHO_ON") {
            // A prompt for input — answer with the secret the user typed.
            helper_in
                .write_all(format!("{secret}\n").as_bytes())
                .await
                .map_err(|e| AgentError::Failed(format!("write response: {e}")))?;
            helper_in.flush().await.ok();
        } else if let Some(msg) = line.strip_prefix("PAM_ERROR_MSG") {
            eprintln!("[Daemon] PAM error:{msg}");
        } else if let Some(msg) = line.strip_prefix("PAM_TEXT_INFO") {
            println!("[Daemon] PAM info:{msg}");
        } else if line == "SUCCESS" {
            verdict = true;
            break;
        } else if line == "FAILURE" {
            verdict = false;
            break;
        }
    }

    let status = helper
        .wait()
        .await
        .map_err(|e| AgentError::Failed(format!("wait helper: {e}")))?;
    Ok(verdict && status.success())
}

/// Full authentication flow for one request: pick the identity, prompt for the
/// secret via the transient UI, then run PAM. Both child processes use
/// `kill_on_drop`, so dropping this future (on cancellation) tears them down.
async fn authenticate(
    action_id: String,
    message: String,
    cookie: String,
    identities: Vec<Subject>,
) -> Result<(), AgentError> {
    let exe = std::env::current_exe()
        .map_err(|e| AgentError::Failed(format!("current_exe: {e}")))?;

    let target_user = choose_target_user(&identities).ok_or_else(|| {
        AgentError::Failed("no identity available to authenticate as".into())
    })?;
    println!("[Daemon] Authenticating action '{action_id}' as user '{target_user}'.");

    // Transient UI subprocess: prints the entered secret to stdout, exits 1 on cancel.
    let child = tokio::process::Command::new(&exe)
        .arg("--ui")
        .arg(&action_id)
        .arg(&message)
        .arg(&target_user)
        .stderr(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| AgentError::Failed(format!("spawn UI: {e}")))?;

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| AgentError::Failed(format!("wait UI: {e}")))?;

    if !output.status.success() {
        return Err(AgentError::Cancelled("user dismissed the prompt".into()));
    }

    // Exact bytes — the UI writes the secret with no trailing newline, so do NOT
    // trim (a password may legitimately contain leading/trailing spaces).
    let secret = String::from_utf8_lossy(&output.stdout).into_owned();
    if secret.is_empty() {
        return Err(AgentError::Cancelled("no secret entered".into()));
    }

    if run_pam(&target_user, &cookie, &secret).await? {
        println!("[Daemon] Authentication succeeded.");
        Ok(())
    } else {
        println!("[Daemon] Authentication failed.");
        Err(AgentError::Failed("authentication failed".into()))
    }
}

// ---- Background D-Bus Daemon ----

/// In-flight authentications, keyed by polkit cookie, each holding a sender that
/// aborts the request when polkit calls `CancelAuthentication`.
type Pending = Arc<Mutex<HashMap<String, oneshot::Sender<()>>>>;

struct PolkitAgent {
    pending: Pending,
}

#[interface(name = "org.freedesktop.PolicyKit1.AuthenticationAgent")]
impl PolkitAgent {
    async fn begin_authentication(
        &self,
        action_id: &str,
        message: &str,
        _icon_name: &str,
        _details: HashMap<String, String>,
        cookie: &str,
        identities: Vec<Subject>,
    ) -> Result<(), AgentError> {
        println!("[Daemon] BeginAuthentication for: {action_id}");

        // Register a cancel channel for this cookie, then race the auth flow
        // against it. If polkit cancels, the auth future is dropped and its
        // `kill_on_drop` children are reaped.
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(cookie.to_string(), tx);

        let outcome = tokio::select! {
            biased;
            _ = rx => {
                println!("[Daemon] Authentication cancelled for: {action_id}");
                Err(AgentError::Cancelled("cancelled by polkit".into()))
            }
            r = authenticate(
                action_id.to_string(),
                message.to_string(),
                cookie.to_string(),
                identities,
            ) => r,
        };

        self.pending.lock().await.remove(cookie);
        outcome
    }

    async fn cancel_authentication(&self, cookie: &str) -> Result<(), AgentError> {
        println!("[Daemon] CancelAuthentication for cookie.");
        if let Some(tx) = self.pending.lock().await.remove(cookie) {
            let _ = tx.send(());
        }
        Ok(())
    }
}
/// Real uid of this process, parsed from `/proc/self/status` (no libc dep).
fn current_uid() -> Option<u32> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("Uid:") {
            // "Uid:\t<real>\t<effective>\t<saved>\t<fs>" — take the real uid.
            return rest.split_whitespace().next()?.parse().ok();
        }
    }
    None
}

/// Ask logind for the active graphical session of the current user.
///
/// Preference order:
///   1. The user's `Display` (graphical) session — login1 already tracks this.
///   2. Scan all sessions: the one owned by our uid that is `Active` and of a
///      graphical type (wayland/x11/mir), or failing that any active one.
async fn discover_active_session(conn: &connection::Connection) -> Option<String> {
    let manager = LoginManagerProxy::new(conn).await.ok()?;
    let uid = current_uid();

    // 1. The user object's Display property is exactly the active graphical session.
    if let Some(uid) = uid {
        if let Ok(user_path) = manager.get_user(uid).await {
            if let Ok(builder) = LoginUserProxy::builder(conn).path(user_path) {
                if let Ok(user) = builder.build().await {
                    if let Ok((sid, _path)) = user.display().await {
                        if !sid.is_empty() {
                            return Some(sid);
                        }
                    }
                }
            }
        }
    }

    // 2. Fall back to scanning every session for an active graphical one.
    let sessions = manager.list_sessions().await.ok()?;
    let mut active_any: Option<String> = None;
    for (sid, suid, _user, _seat, path) in sessions {
        if let Some(uid) = uid {
            if suid != uid {
                continue;
            }
        }
        let Ok(builder) = LoginSessionProxy::builder(conn).path(path) else {
            continue;
        };
        let Ok(session) = builder.build().await else {
            continue;
        };
        let active = session.active().await.unwrap_or(false);
        if !active {
            continue;
        }
        let stype = session.type_().await.unwrap_or_default();
        if matches!(stype.as_str(), "wayland" | "x11" | "mir") {
            return Some(sid); // active + graphical — the one we want.
        }
        active_any.get_or_insert(sid); // remember as a weaker fallback.
    }
    active_any
}

/// Resolve the logind session to register the agent for, preferring live
/// logind discovery and degrading gracefully to cgroup/env heuristics.
async fn resolve_session_id(conn: &connection::Connection) -> String {
    if let Some(sid) = discover_active_session(conn).await {
        println!("[Daemon] Discovered active graphical session via logind: {sid}");
        return sid;
    }
    eprintln!("[Daemon] logind discovery failed; falling back to cgroup/env.");
    get_true_session_id()
}

fn get_true_session_id() -> String {
    // Attempt to extract the true logind session from the cgroup v2 tree
    if let Ok(cgroup) = std::fs::read_to_string("/proc/self/cgroup") {
        if let Some(scope) = cgroup
            .split('/')
            .find(|s| s.starts_with("session-") && s.ends_with(".scope"))
        {
            return scope
                .trim_start_matches("session-")
                .trim_end_matches(".scope")
                .to_string();
        }
    }

    // Fallback to the environment variable if cgroup parsing fails
    std::env::var("XDG_SESSION_ID").unwrap_or_else(|_| {
        eprintln!("WARNING: Could not determine true logind session.");
        String::new()
    })
}
async fn run_daemon() -> Result<(), Box<dyn Error>> {
    let connection = connection::Connection::system().await?;
    let agent_path = "/org/custom/PolkitAgent";

    let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
    connection
        .object_server()
        .at(agent_path, PolkitAgent { pending })
        .await?;

    // Discover the active graphical session automatically (no env var needed).
    let session_id = resolve_session_id(&connection).await;
    println!("Registering agent under logind session: {}", session_id);

    let mut details = HashMap::new();
    details.insert(
        "session-id".to_string(),
        zbus::zvariant::Value::from(session_id.as_str())
            .try_to_owned()
            .unwrap(),
    );

    let subject = Subject {
        subject_kind: "unix-session".to_string(),
        subject_details: details,
    };

    let authority_proxy = AuthorityProxy::new(&connection).await?;

    // Register the agent for the entire graphical session
    authority_proxy
        .register_authentication_agent(&subject, "en_US.UTF-8", agent_path)
        .await?;

    println!("Session D-Bus Agent registered successfully. Holding event loop...");

    // Hold until asked to stop (systemd SIGTERM, or Ctrl-C), then unregister so
    // polkit drops us immediately instead of waiting for the bus name to vanish.
    wait_for_shutdown().await;
    println!("Shutting down; unregistering agent.");
    let _ = authority_proxy
        .unregister_authentication_agent(&subject, agent_path)
        .await;

    Ok(())
}

/// Resolve when the process receives SIGTERM (systemd stop) or SIGINT (Ctrl-C).
async fn wait_for_shutdown() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut term = match signal(SignalKind::terminate()) {
        Ok(s) => s,
        Err(_) => {
            std::future::pending::<()>().await;
            return;
        }
    };
    let mut int = match signal(SignalKind::interrupt()) {
        Ok(s) => s,
        Err(_) => {
            std::future::pending::<()>().await;
            return;
        }
    };
    tokio::select! {
        _ = term.recv() => {}
        _ = int.recv() => {}
    }
}

// ---- Transient UI Application ----

struct AgentUI {
    action_id: String,
    message: String,
    username: String,
    password_input: String,
}

#[derive(Debug, Clone)]
enum Message {
    PasswordChanged(String),
    SubmitAuthentication,
    CancelAuthentication,
}

impl AgentUI {
    fn title(&self) -> String {
        String::from("System Authentication Required")
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PasswordChanged(val) => {
                self.password_input = val;
            }
            Message::SubmitAuthentication => {
                // Print the raw string into the pipe for the parent tokio process
                print!("{}", self.password_input);
                let _ = std::io::Write::flush(&mut std::io::stdout());
                std::process::exit(0);
            }
            Message::CancelAuthentication => {
                std::process::exit(1);
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<Message> {
        column![
            text("Authentication Required").size(22),
            text(&self.message).size(15),
            text(format!("Authenticating as: {}", self.username)).size(13),
            text(format!("Action: {}", self.action_id)).size(11),
            text_input(&format!("Password for {}", self.username), &self.password_input)
                .on_input(Message::PasswordChanged)
                .on_submit(Message::SubmitAuthentication)
                .secure(true),
            row![
                button("Authenticate").on_press(Message::SubmitAuthentication),
                button("Cancel").on_press(Message::CancelAuthentication),
            ]
            .spacing(12)
        ]
        .spacing(12)
        .padding(24)
        .align_x(Alignment::Center)
        .into()
    }
}

fn run_ui(action_id: String, message: String, username: String) -> iced::Result {
    // 1. The boot closure comes first. It takes 0 arguments and returns (State, Task).
    // The variables are cloned inside the closure to satisfy the `Fn` trait bound.
    iced::application(
        move || {
            (
                AgentUI {
                    action_id: action_id.clone(),
                    message: message.clone(),
                    username: username.clone(),
                    password_input: String::new(),
                },
                Task::none(),
            )
        },
        AgentUI::update,
        AgentUI::view,
    )
    // 2. Chain the title handler
    .title(AgentUI::title)
    // 3. Trigger the loop natively
    .run()
}

// ---- Bootloader ----

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && args[1] == "--ui" {
        let action_id = args
            .get(2)
            .cloned()
            .unwrap_or_else(|| "Unknown".to_string());
        let message = args
            .get(3)
            .cloned()
            .unwrap_or_else(|| "Authentication request".to_string());
        let username = args.get(4).cloned().unwrap_or_else(|| "root".to_string());
        // Submit exits 0 (secret on stdout); cancel exits 1. A failure to even bring
        // up the window is a hard error, not a silent "empty secret" → exit non-zero.
        if run_ui(action_id, message, username).is_err() {
            std::process::exit(2);
        }
        return Ok(());
    }

    // Parent Process: Create a dedicated local tokio runtime so it never tangles with Iced.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        if let Err(e) = run_daemon().await {
            eprintln!("Daemon crashed: {}", e);
        }
    });

    Ok(())
}
