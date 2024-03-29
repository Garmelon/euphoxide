//! A single instance of a bot in a single room.
//!
//! See [`Instance`] for more details.

use std::convert::Infallible;
use std::fmt;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cookie::{Cookie, CookieJar};
use tokio::select;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::http::{HeaderValue, StatusCode};

use crate::api::packet::ParsedPacket;
use crate::api::{Auth, AuthOption, Data, Nick};
use crate::conn::{self, Conn, ConnTx, State};

macro_rules! ilog {
    ( $conf:expr, $target:expr, $($arg:tt)+ ) => {
        ::log::log!(
            target: &format!("euphoxide::live::{}", $conf.name),
            $target,
            $($arg)+
        );
    };
}

macro_rules! idebug {
    ( $conf:expr, $($arg:tt)+ ) => {
        ilog!($conf, ::log::Level::Debug, $($arg)+);
    };
}

macro_rules! iinfo {
    ( $conf:expr, $($arg:tt)+ ) => {
        ilog!($conf, ::log::Level::Info, $($arg)+);
    };
}

macro_rules! iwarn {
    ( $conf:expr, $($arg:tt)+ ) => {
        ilog!($conf, ::log::Level::Warn, $($arg)+);
    };
}

/// Settings that are usually shared between all instances connecting to a
/// specific server.
#[derive(Clone)]
pub struct ServerConfig {
    /// How long to wait for the server until an operation is considered timed
    /// out.
    ///
    /// This timeout applies to waiting for reply packets to command packets
    /// sent by the client, as well as operations like connecting or closing a
    /// connection.
    pub timeout: Duration,
    /// How long to wait until reconnecting after an unsuccessful attempt to
    /// connect.
    pub reconnect_delay: Duration,
    /// Domain name, to be used with [`Conn::connect`].
    pub domain: String,
    /// Cookies to use when connecting. They are updated with the server's reply
    /// after successful connection attempts.
    pub cookies: Arc<Mutex<CookieJar>>,
}

impl ServerConfig {
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn reconnect_delay(mut self, reconnect_delay: Duration) -> Self {
        self.reconnect_delay = reconnect_delay;
        self
    }

    pub fn domain<S: ToString>(mut self, domain: S) -> Self {
        self.domain = domain.to_string();
        self
    }

    pub fn cookies(mut self, cookies: Arc<Mutex<CookieJar>>) -> Self {
        self.cookies = cookies;
        self
    }

    pub fn room<S: ToString>(self, room: S) -> InstanceConfig {
        InstanceConfig::new(self, room)
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            reconnect_delay: Duration::from_secs(30),
            domain: "euphoria.leet.nu".to_string(),
            cookies: Arc::new(Mutex::new(CookieJar::new())),
        }
    }
}

struct Hidden;

impl fmt::Debug for Hidden {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<hidden>")
    }
}

impl fmt::Debug for ServerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerConfig")
            .field("timeout", &self.timeout)
            .field("reconnect_delay", &self.reconnect_delay)
            .field("domain", &self.domain)
            .field("cookies", &Hidden)
            .finish()
    }
}

/// Settings that are usually specific to a single instance.
#[derive(Debug, Clone)]
pub struct InstanceConfig {
    pub server: ServerConfig,
    /// Unique name of this instance.
    pub name: String,
    /// Room name, to be used with [`Conn::connect`].
    pub room: String,
    /// Whether the instance should connect as human or bot.
    pub human: bool,
    /// Username to set upon connecting.
    pub username: Option<String>,
    /// Whether to set the username even if the server reports that the session
    /// already has a username set.
    pub force_username: bool,
    /// Password to use if room requires authentication.
    pub password: Option<String>,
}

impl InstanceConfig {
    pub fn new<S: ToString>(server: ServerConfig, room: S) -> Self {
        Self {
            server,
            name: room.to_string(),
            room: room.to_string(),
            human: false,
            username: None,
            force_username: false,
            password: None,
        }
    }

    pub fn name<S: ToString>(mut self, name: S) -> Self {
        self.name = name.to_string();
        self
    }

    pub fn human(mut self, human: bool) -> Self {
        self.human = human;
        self
    }

    pub fn username<S: ToString>(mut self, username: Option<S>) -> Self {
        self.username = username.map(|s| s.to_string());
        self
    }

    pub fn force_username(mut self, force_username: bool) -> Self {
        self.force_username = force_username;
        self
    }

    pub fn password<S: ToString>(mut self, password: Option<S>) -> Self {
        self.password = password.map(|s| s.to_string());
        self
    }

    /// Create a new instance using this config.
    ///
    /// See [`Instance::new`] for more details.
    pub fn build<F>(self, on_event: F) -> Instance
    where
        F: Fn(Event) + Send + Sync + 'static,
    {
        Instance::new(self, on_event)
    }
}

/// Snapshot of a [`Conn`]'s state immediately after receiving a packet.
#[derive(Debug, Clone)]
pub struct ConnSnapshot {
    pub conn_tx: ConnTx,
    pub state: State,
}

impl ConnSnapshot {
    fn from_conn(conn: &Conn) -> Self {
        Self {
            conn_tx: conn.tx().clone(),
            state: conn.state().clone(),
        }
    }
}

// Most of the time, the largest variant (`Packet`) is sent. The size of this
// enum is not critical anyways since it's not constructed that often.
#[allow(clippy::large_enum_variant)]
/// An event emitted by an [`Instance`].
///
/// Events are emitted by a single instance following this schema, written in
/// pseudo-regex syntax:
/// ```text
/// (Connecting (Connected Packet*)? Disconnected)* Stopped
/// ```
///
/// In particular, this means that every [`Self::Connecting`] is always followed
/// by exactly one [`Self::Disconnected`], and that [`Self::Stopped`] is always
/// the last event and is always sent exactly once per instance.
#[derive(Debug)]
pub enum Event {
    Connecting(InstanceConfig),
    Connected(InstanceConfig, ConnSnapshot),
    Packet(InstanceConfig, ParsedPacket, ConnSnapshot),
    Disconnected(InstanceConfig),
    Stopped(InstanceConfig),
}

impl Event {
    pub fn config(&self) -> &InstanceConfig {
        match self {
            Self::Connecting(config) => config,
            Self::Connected(config, _) => config,
            Self::Packet(config, _, _) => config,
            Self::Disconnected(config) => config,
            Self::Stopped(config) => config,
        }
    }
}

enum Request {
    GetConnTx(oneshot::Sender<ConnTx>),
    Stop,
}

/// An error that occurred inside an [`Instance`] while it was running.
enum RunError {
    StoppedManually,
    InstanceDropped,
    CouldNotConnect(conn::Error),
    Conn(conn::Error),
}

/// A single instance of a bot in a single room.
///
/// The instance automatically connects to its room once it is created, and it
/// reconnects when it loses connection. If the room requires authentication and
/// a password is given, the instance automatically authenticates. If a nick is
/// given, the instance sets its nick upon joining the room.
///
/// An instance has a unique name used for logging and identifying the instance.
/// The room name can be used as the instance name if there is never more than
/// one instance per room.
///
/// An instance can be created using [`Instance::new`] or using
/// [`InstanceConfig::build`].
///
/// An instance can be stopped using [`Instance::stop`] or by dropping it. In
/// either case, the last event the instance sends will be an
/// [`Event::Stopped`]. If it is not stopped using one of these two ways, it
/// will continue to run and reconnect indefinitely.
#[derive(Debug, Clone)]
pub struct Instance {
    config: InstanceConfig,
    request_tx: mpsc::UnboundedSender<Request>,
    // In theory, request_tx should be sufficient as canary, but I'm not sure
    // exactly how to check it during the reconnect timeout.
    _canary_tx: mpsc::UnboundedSender<Infallible>,
}

impl Instance {
    // Previously, the event callback was asynchronous and would return a result. It
    // was called in-line to calling Conn::recv. The idea was that the instance
    // would stop if the event handler returned Err. This was, however, not even
    // implemented correctly and the instance would just reconnect.
    //
    // The new event handler is synchronous. This way, it becomes harder to
    // accidentally block Conn::recv, for example by waiting for a channel with
    // limited capacity. If async code must be executed upon receiving a command,
    // the user can start a task from inside the handler.
    //
    // The new event handler does not return anything. This makes the code nicer. In
    // the use cases I'm thinking of, it should not be a problem: If the event
    // handler encounters errors, there's usually other ways to tell the same. Make
    // the event handler ignore the errors and stop the instance in that other way.

    /// Create a new instance based on an [`InstanceConfig`].
    ///
    /// The `on_event` parameter is called whenever the instance wants to emit
    /// an [`Event`]. It must not block for long. See [`Event`] for more details
    /// on the events and the order in which they are emitted.
    ///
    /// [`InstanceConfig::build`] can be used in place of this function.
    pub fn new<F>(config: InstanceConfig, on_event: F) -> Self
    where
        F: Fn(Event) + Send + Sync + 'static,
    {
        idebug!(config, "Created with config {config:?}");

        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let (canary_tx, canary_rx) = mpsc::unbounded_channel();

        tokio::spawn(Self::run::<F>(
            config.clone(),
            on_event,
            request_rx,
            canary_rx,
        ));

        Self {
            config,
            request_tx,
            _canary_tx: canary_tx,
        }
    }

    pub fn config(&self) -> &InstanceConfig {
        &self.config
    }

    /// Retrieve the instance's current connection.
    ///
    /// Returns `None` if the instance is currently not connected, or has
    /// stopped running.
    pub async fn conn_tx(&self) -> Option<ConnTx> {
        let (tx, rx) = oneshot::channel();
        let _ = self.request_tx.send(Request::GetConnTx(tx));
        rx.await.ok()
    }

    /// Stop the instance.
    ///
    /// For more info on stopping instances, see [`Instance`].
    pub fn stop(&self) {
        let _ = self.request_tx.send(Request::Stop);
    }

    /// Whether this instance is stopped.
    ///
    /// For more info on stopping instances, see [`Instance`].
    pub fn stopped(&self) -> bool {
        self.request_tx.is_closed()
    }

    async fn run<F: Fn(Event)>(
        config: InstanceConfig,
        on_event: F,
        request_rx: mpsc::UnboundedReceiver<Request>,
        mut canary_rx: mpsc::UnboundedReceiver<Infallible>,
    ) {
        select! {
            _ = Self::stay_connected(&config, &on_event, request_rx) => (),
            _ = canary_rx.recv() => { idebug!(config, "Instance dropped"); },
        }
        on_event(Event::Stopped(config))
    }

    async fn stay_connected<F: Fn(Event)>(
        config: &InstanceConfig,
        on_event: &F,
        mut request_rx: mpsc::UnboundedReceiver<Request>,
    ) {
        loop {
            idebug!(config, "Connecting...");

            on_event(Event::Connecting(config.clone()));
            let result = Self::run_once::<F>(config, on_event, &mut request_rx).await;
            on_event(Event::Disconnected(config.clone()));

            let connected = match result {
                Ok(()) => {
                    idebug!(config, "Connection closed normally");
                    true
                }
                Err(RunError::StoppedManually) => {
                    idebug!(config, "Instance stopped manually");
                    break;
                }
                Err(RunError::InstanceDropped) => {
                    idebug!(config, "Instance dropped");
                    break;
                }
                Err(RunError::CouldNotConnect(conn::Error::Tungstenite(
                    tungstenite::Error::Http(response),
                ))) if response.status() == StatusCode::NOT_FOUND => {
                    iwarn!(config, "Failed to connect: room does not exist");
                    break;
                }
                Err(RunError::CouldNotConnect(err)) => {
                    iwarn!(config, "Failed to connect: {err}");
                    false
                }
                Err(RunError::Conn(err)) => {
                    iwarn!(config, "An error occurred: {err}");
                    true
                }
            };

            if !connected {
                let s = config.server.reconnect_delay.as_secs();
                idebug!(config, "Waiting {s} seconds before reconnecting");
                tokio::time::sleep(config.server.reconnect_delay).await;
            }
        }
    }

    fn get_cookies(config: &InstanceConfig) -> HeaderValue {
        let guard = config.server.cookies.lock().unwrap();
        let cookies = guard
            .iter()
            .map(|c| format!("{}", c.stripped()))
            .collect::<Vec<_>>()
            .join("; ");
        drop(guard);
        cookies.try_into().unwrap()
    }

    fn set_cookies(config: &InstanceConfig, cookies: Vec<HeaderValue>) {
        idebug!(config, "Updating cookies");
        let mut guard = config.server.cookies.lock().unwrap();

        for cookie in cookies {
            if let Ok(cookie) = cookie.to_str() {
                if let Ok(cookie) = Cookie::from_str(cookie) {
                    guard.add(cookie);
                }
            }
        }
    }

    async fn run_once<F: Fn(Event)>(
        config: &InstanceConfig,
        on_event: &F,
        request_rx: &mut mpsc::UnboundedReceiver<Request>,
    ) -> Result<(), RunError> {
        let (mut conn, cookies) = Conn::connect(
            &config.server.domain,
            &config.room,
            config.human,
            Some(Self::get_cookies(config)),
            config.server.timeout,
        )
        .await
        .map_err(RunError::CouldNotConnect)?;

        Self::set_cookies(config, cookies);
        on_event(Event::Connected(
            config.clone(),
            ConnSnapshot::from_conn(&conn),
        ));

        let conn_tx = conn.tx().clone();
        select! {
            r = Self::receive::<F>(config, &mut conn, on_event) => r,
            r = Self::handle_requests(request_rx, &conn_tx) => Err(r),
        }
    }

    async fn receive<F: Fn(Event)>(
        config: &InstanceConfig,
        conn: &mut Conn,
        on_event: &F,
    ) -> Result<(), RunError> {
        loop {
            let packet = conn.recv().await.map_err(RunError::Conn)?;
            let snapshot = ConnSnapshot::from_conn(conn);

            match &packet.content {
                Ok(Data::SnapshotEvent(snapshot)) => {
                    if let Some(username) = &config.username {
                        if config.force_username || snapshot.nick.is_none() {
                            idebug!(config, "Setting nick to username {username}");
                            let name = username.to_string();
                            conn.tx().send_only(Nick { name });
                        } else if let Some(nick) = &snapshot.nick {
                            idebug!(config, "Not setting nick, already set to {nick}");
                        }
                    }
                }
                Ok(Data::BounceEvent(_)) => {
                    if let Some(password) = &config.password {
                        idebug!(config, "Authenticating with password");
                        let cmd = Auth {
                            r#type: AuthOption::Passcode,
                            passcode: Some(password.to_string()),
                        };
                        conn.tx().send_only(cmd);
                    } else {
                        iwarn!(config, "Auth required but no password configured");
                    }
                }
                Ok(Data::DisconnectEvent(ev)) => {
                    if ev.reason == "authentication changed" {
                        iinfo!(config, "Disconnected because {}", ev.reason);
                    } else {
                        iwarn!(config, "Disconnected because {}", ev.reason);
                    }
                }
                _ => {}
            }

            on_event(Event::Packet(config.clone(), packet, snapshot));
        }
    }

    async fn handle_requests(
        request_rx: &mut mpsc::UnboundedReceiver<Request>,
        conn_tx: &ConnTx,
    ) -> RunError {
        while let Some(request) = request_rx.recv().await {
            match request {
                Request::GetConnTx(tx) => {
                    let _ = tx.send(conn_tx.clone());
                }
                Request::Stop => return RunError::StoppedManually,
            }
        }
        RunError::InstanceDropped
    }
}
