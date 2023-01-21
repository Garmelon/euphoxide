//! A single instance of a bot in a single room.
//!
//! See [`Instance`] for more details.

// TODO Cookies
// TODO Logging

use std::future::Future;
use std::time::Duration;

use tokio::select;
use tokio::sync::{mpsc, oneshot};

use crate::api::packet::ParsedPacket;
use crate::api::{Auth, AuthOption, Data, Nick};
use crate::conn::{Conn, ConnTx, State};

const EUPH_DOMAIN: &str = "euphoria.io";
const TIMEOUT: Duration = Duration::from_secs(30);
const RECONNECT: Duration = Duration::from_secs(30);

/// Settings that are not changed over the life time of the instance.
#[derive(Debug, Clone)]
pub struct Config {
    /// Unique name of this instance.
    pub name: String,
    /// Domain name, to be used with [`euphoxide::connect`].
    pub domain: String,
    /// Room name, to be used with [`euphoxide::connect`].
    pub room: String,
    /// Whether the instance should connect as human or bot.
    pub human: bool,
    /// Username to set upon connecting.
    pub username: Option<String>,
    /// Password to use if room requires authentication.
    pub password: Option<String>,
}

impl Config {
    pub fn new<S: ToString>(room: S) -> Self {
        Self {
            name: room.to_string(),
            domain: EUPH_DOMAIN.to_string(),
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

    pub fn domain<S: ToString>(mut self, domain: S) -> Self {
        self.domain = domain.to_string();
        self
    }

    pub fn human(mut self) -> Self {
        self.human = true;
        self
    }

    pub fn bot(mut self) -> Self {
        self.human = false;
        self
    }

    pub fn username<S: ToString>(mut self, username: S) -> Self {
        self.username = Some(username.to_string());
        self
    }

    pub fn password<S: ToString>(mut self, password: S) -> Self {
        self.password = Some(password.to_string());
        self
    }

    pub fn build<F, Fut>(self, on_event: F) -> Instance
    where
        F: FnMut(Event) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), ()>> + Send + 'static,
    {
        Instance::new(self, on_event)
    }
}

/// Snapshot of an instance at a specific point in time, usually after just
/// receiving a packet.
pub struct Snapshot {
    pub config: Config,
    pub conn_tx: ConnTx,
    pub state: State,
}

pub struct Event {
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
pub struct Instance {
    config: Config,
    request_tx: mpsc::UnboundedSender<oneshot::Sender<ConnTx>>,
}

impl Instance {
    pub fn new<F, Fut>(config: Config, on_event: F) -> Self
    where
        F: FnMut(Event) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), ()>> + Send + 'static,
    {
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        tokio::spawn(Self::run::<F, Fut>(config.clone(), on_event, request_rx));
        Self { config, request_tx }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub async fn conn_tx(&self) -> Option<ConnTx> {
        let (tx, rx) = oneshot::channel();
        let _ = self.request_tx.send(tx);
        rx.await.ok()
    }

    async fn run<F, Fut>(
        config: Config,
        mut on_event: F,
        mut request_rx: mpsc::UnboundedReceiver<oneshot::Sender<ConnTx>>,
    ) where
        F: FnMut(Event) -> Fut,
        Fut: Future<Output = Result<(), ()>>,
    {
        loop {
            Self::run_once::<F, Fut>(&config, &mut on_event, &mut request_rx).await;
            tokio::time::sleep(RECONNECT).await;
        }
    }

    async fn run_once<F, Fut>(
        config: &Config,
        on_event: &mut F,
        request_rx: &mut mpsc::UnboundedReceiver<oneshot::Sender<ConnTx>>,
    ) -> Option<()>
    where
        F: FnMut(Event) -> Fut,
        Fut: Future<Output = Result<(), ()>>,
    {
        let (mut conn, _) =
            Conn::connect(&config.domain, &config.room, config.human, None, TIMEOUT)
                .await
                .ok()?;
        let conn_tx = conn.tx().clone();

        select! {
            _ = Self::receive::<F, Fut>(config, &mut conn, on_event) => (),
            _ = Self::handle_requests(request_rx, &conn_tx) => (),
        }

        Some(())
    }

    async fn receive<F, Fut>(config: &Config, conn: &mut Conn, on_event: &mut F)
    where
        F: FnMut(Event) -> Fut,
        Fut: Future<Output = Result<(), ()>>,
    {
        while let Ok(packet) = conn.recv().await {
            let event = Event {
                packet,
                snapshot: Snapshot {
                    config: config.clone(),
                    conn_tx: conn.tx().clone(),
                    state: conn.state().clone(),
                },
            };

            match &event.packet.content {
                Ok(Data::SnapshotEvent(_)) => {
                    if let Some(username) = &config.username {
                        let name = username.to_string();
                        let _ = conn.tx().send(Nick { name });
                    }
                }
                Ok(Data::BounceEvent(_)) => {
                    if let Some(password) = &config.password {
                        let cmd = Auth {
                            r#type: AuthOption::Passcode,
                            passcode: Some(password.to_string()),
                        };
                        let _ = conn.tx().send(cmd);
                    } else {
                        break;
                    }
                }
                _ => {}
            }

            if on_event(event).await.is_err() {
                break;
            }
        }
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
