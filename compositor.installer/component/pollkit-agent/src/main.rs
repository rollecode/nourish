use iced::widget::{button, column, row, text, text_input};
use iced::{Alignment, Element, Task};
use std::collections::HashMap;
use std::error::Error;
use zbus::{connection, interface, proxy};

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

// ---- Background D-Bus Daemon ----

struct PolkitAgent;

#[interface(name = "org.freedesktop.PolicyKit1.AuthenticationAgent")]
impl PolkitAgent {
    async fn begin_authentication(
        &self,
        action_id: &str,
        message: &str,
        _icon_name: &str,
        _details: HashMap<String, String>,
        cookie: &str,
        _identities: Vec<Subject>,
    ) -> Result<(), zbus::fdo::Error> {
        let exe =
            std::env::current_exe().map_err(|_| zbus::fdo::Error::Failed("Missing Exe".into()))?;

        println!(
            "\n[Daemon] Incoming D-Bus Request! Spawning UI prompt for: {}",
            action_id
        );

        let mut child = tokio::process::Command::new(exe)
            .arg("--ui")
            .arg(action_id)
            .arg(message)
            .stderr(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        if output.status.success() {
            let password = String::from_utf8_lossy(&output.stdout).trim().to_string();

            if !password.is_empty() {
                // --- FIX: DYNAMIC USER EXTRACTION ---
                let mut target_user = String::from("root"); // Default fallback

                if let Some(identity) = _identities.first() {
                    if identity.subject_kind == "unix-user" {
                        if let Some(uid_val) = identity.subject_details.get("uid") {
                            // Explicitly try to unwrap the zvariant value as a u32
                            if let Ok(uid) = u32::try_from(uid_val) {
                                let uid_str = uid.to_string();

                                if let Ok(output) = std::process::Command::new("id")
                                    .arg("-nu")
                                    .arg(&uid_str)
                                    .output()
                                {
                                    if output.status.success() {
                                        target_user = String::from_utf8_lossy(&output.stdout)
                                            .trim()
                                            .to_string();
                                    }
                                }
                            } else {
                                println!("[Daemon] Warning: UID variant was not a valid u32.");
                            }
                        }
                    }
                }

                println!(
                    "[Daemon] Target user determined as: '{}'. Forwarding to PAM...",
                    target_user
                );

                let mut helper =
                    tokio::process::Command::new("/usr/lib/polkit-1/polkit-agent-helper-1")
                        .arg(&target_user)
                        .stdin(std::process::Stdio::piped())
                        .stdout(std::process::Stdio::piped()) // Capture stdout to sequence the prompt
                        .stderr(std::process::Stdio::inherit())
                        .spawn()
                        .map_err(|e| {
                            zbus::fdo::Error::Failed(format!("Failed to spawn helper: {}", e))
                        })?;

                if let (Some(helper_out), Some(mut helper_in)) =
                    (helper.stdout.take(), helper.stdin.take())
                {
                    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

                    // polkit-agent-helper-1 protocol (the helper reads from stdin, NOT an
                    // env var): the FIRST line on stdin must be the cookie, then a PAM
                    // conversation follows line-by-line, ending in SUCCESS/FAILURE on stdout.
                    let _ = helper_in
                        .write_all(format!("{}\n", cookie).as_bytes())
                        .await;
                    let _ = helper_in.flush().await;

                    let mut lines = BufReader::new(helper_out).lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        let line = line.trim();
                        if line.starts_with("PAM_PROMPT_ECHO_OFF")
                            || line.starts_with("PAM_PROMPT_ECHO_ON")
                        {
                            // PAM is asking for input — feed it the collected password.
                            let _ = helper_in
                                .write_all(format!("{}\n", password).as_bytes())
                                .await;
                            let _ = helper_in.flush().await;
                        } else if line == "SUCCESS" || line == "FAILURE" {
                            break; // helper reports its verdict and exits.
                        }
                        // PAM_ERROR_MSG / PAM_TEXT_INFO lines are informational — ignore.
                    }
                }

                let helper_status = helper
                    .wait()
                    .await
                    .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

                if helper_status.success() {
                    println!("[Daemon] Authentication verified successfully.");
                } else {
                    println!("[Daemon] Authentication rejected by PAM.");
                }
            }
        } else {
            println!("[Daemon] UI prompt cancelled or failed.");
        }

        Ok(())
    }

    async fn cancel_authentication(&self, _cookie: &str) -> Result<(), zbus::fdo::Error> {
        println!("[Daemon] Request cancelled by authoritative daemon.");
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

    connection
        .object_server()
        .at(agent_path, PolkitAgent)
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
    std::future::pending::<()>().await;

    Ok(())
}

// ---- Transient UI Application ----

struct AgentUI {
    action_id: String,
    message: String,
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
            text(format!("Action: {}", self.action_id)).size(13),
            text(&self.message).size(15),
            text_input("Enter Password", &self.password_input)
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

fn run_ui(action_id: String, message: String) -> iced::Result {
    // 1. The boot closure comes first. It takes 0 arguments and returns (State, Task).
    // The variables are cloned inside the closure to satisfy the `Fn` trait bound.
    iced::application(
        move || {
            (
                AgentUI {
                    action_id: action_id.clone(),
                    message: message.clone(),
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
        let _ = run_ui(action_id, message);
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
