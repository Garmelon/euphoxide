//! A single instance of a bot in a single room.
//!
//! See [`Instance`] for more details.

use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cookie::{Cookie, CookieJar};
use log::{debug, warn};
use tokio::select;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::http::HeaderValue;

use crate::api::packet::ParsedPacket;
use crate::api::{Auth, AuthOption, Data, Nick};
use crate::conn::{self, Conn, ConnTx, State};

/// Settings that are usually shared between all instances connecting to a
/// specific server.
#[derive(Debug, Clone)]
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
    /// Domain name, to be used with [`euphoxide::connect`].
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
            domain: "euphoria.io".to_string(),
            cookies: Arc::new(Mutex::new(CookieJar::new())),
        }
    }
}

/// Settings that are usually specific to a single instance.
#[derive(Debug, Clone)]
pub struct InstanceConfig {
    pub server: ServerConfig,
    /// Unique name of this instance.
    pub name: String,
    /// Room name, to be used with [`euphoxide::connect`].
    pub room: String,
    /// Whether the instance should connect as human or bot.
    pub human: bool,
    /// Username to set upon connecting.
    pub username: Option<String>,
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

    pub fn password<S: ToString>(mut self, password: Option<S>) -> Self {
        self.password = password.map(|s| s.to_string());
        self
    }

    pub fn build<F>(self, on_event: F) -> Instance
    where
        F: Fn(Event) + Send + Sync + 'static,
    {
        Instance::new(self, on_event)
    }
}

/// Snapshot of a [`Conn`]'s state immediately after receiving a packet.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub conn_tx: ConnTx,
    pub state: State,
}

#[derive(Debug)]
pub struct Event {
    pub config: InstanceConfig,
    pub packet: ParsedPacket,
    pub snapshot: Snapshot,
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
#[derive(Debug)]
pub struct Instance {
    // TODO Share Arc<InstanceConfig> instead of cloning InstanceConfig everywhere
    config: InstanceConfig,
    request_tx: mpsc::UnboundedSender<oneshot::Sender<ConnTx>>,
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

    pub fn new<F>(config: InstanceConfig, on_event: F) -> Self
    where
        F: Fn(Event) + Send + Sync + 'static,
    {
        debug!("{}: Created with config {config:?}", config.name);
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        tokio::spawn(Self::run::<F>(config.clone(), on_event, request_rx));
        Self { config, request_tx }
    }

    pub fn config(&self) -> &InstanceConfig {
        &self.config
    }

    pub async fn conn_tx(&self) -> Option<ConnTx> {
        let (tx, rx) = oneshot::channel();
        let _ = self.request_tx.send(tx);
        rx.await.ok()
    }

    async fn run<F>(
        config: InstanceConfig,
        on_event: F,
        mut request_rx: mpsc::UnboundedReceiver<oneshot::Sender<ConnTx>>,
    ) where
        F: Fn(Event),
    {
        // TODO Only delay reconnecting if previous reconnect attempt failed
        loop {
            Self::run_once::<F>(&config, &on_event, &mut request_rx).await;
            debug!(
                "{}: Waiting {} seconds before reconnecting",
                config.name,
                config.server.reconnect_delay.as_secs(),
            );
            tokio::time::sleep(config.server.reconnect_delay).await;
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
        debug!("Updating cookies");
        let mut guard = config.server.cookies.lock().unwrap();

        for cookie in cookies {
            if let Ok(cookie) = cookie.to_str() {
                if let Ok(cookie) = Cookie::from_str(cookie) {
                    guard.add(cookie);
                }
            }
        }
    }

    async fn run_once<F>(
        config: &InstanceConfig,
        on_event: &F,
        request_rx: &mut mpsc::UnboundedReceiver<oneshot::Sender<ConnTx>>,
    ) -> Option<()>
    where
        F: Fn(Event),
    {
        debug!("{}: Connecting...", config.name);
        let (mut conn, cookies) = Conn::connect(
            &config.server.domain,
            &config.room,
            config.human,
            Some(Self::get_cookies(config)),
            config.server.timeout,
        )
        .await
        .ok()?;
        Self::set_cookies(config, cookies);

        let conn_tx = conn.tx().clone();
        let result = select! {
            r = Self::receive::<F>(config, &mut conn, on_event) => r,
            _ = Self::handle_requests(request_rx, &conn_tx) => Ok(()),
        };
        if let Err(err) = result {
            if matches!(err, conn::Error::ConnectionClosed) {
                debug!("{}: Connection closed, reconnecting", config.name);
            } else {
                warn!("{}: An error occurred, reconnecting: {err}", config.name);
            }
        }

        Some(())
    }

    async fn receive<F>(config: &InstanceConfig, conn: &mut Conn, on_event: &F) -> conn::Result<()>
    where
        F: Fn(Event),
    {
        loop {
            let packet = conn.recv().await?;
            let snapshot = Snapshot {
                conn_tx: conn.tx().clone(),
                state: conn.state().clone(),
            };
            let event = Event {
                config: config.clone(),
                packet,
                snapshot,
            };

            match &event.packet.content {
                Ok(Data::SnapshotEvent(_)) => {
                    if let Some(username) = &config.username {
                        debug!("{}: Setting nick to username {}", config.name, username);
                        let name = username.to_string();
                        let _ = conn.tx().send(Nick { name });
                    }
                }
                Ok(Data::BounceEvent(_)) => {
                    if let Some(password) = &config.password {
                        debug!("{}: Authenticating with password", config.name);
                        let cmd = Auth {
                            r#type: AuthOption::Passcode,
                            passcode: Some(password.to_string()),
                        };
                        let _ = conn.tx().send(cmd);
                    } else {
                        warn!("{}: Auth required but no password configured", config.name);
                        break;
                    }
                }
                _ => {}
            }

            on_event(event);
        }

        Ok(())
    }

    async fn handle_requests(
        request_rx: &mut mpsc::UnboundedReceiver<oneshot::Sender<ConnTx>>,
        conn_tx: &ConnTx,
    ) {
        while let Some(request) = request_rx.recv().await {
            let _ = request.send(conn_tx.clone());
        }
    }
}
