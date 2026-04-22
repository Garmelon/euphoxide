use std::{fmt, result, str::FromStr, sync::Arc};

use cookie::Cookie;
use euphoxide::{
    api::{Auth, AuthOption, Data, Nick, ParsedPacket},
    client::{ClientConn, ClientConnHandle, State},
};
use jiff::Timestamp;
use log::warn;
use tokio::{
    select,
    sync::{mpsc, oneshot},
};
use tokio_tungstenite::tungstenite::{
    self,
    http::{HeaderValue, StatusCode},
};

use crate::{ClientBuilder, ClientConfig, ServerConfig};

enum Error {
    Stopped,
    NoReferences,
    AuthRequired,
    InvalidPassword,
    OutOfJoinAttempts,
    Euphoxide(euphoxide::Error),
}

impl Error {
    fn is_fatal(&self) -> bool {
        match self {
            Self::Stopped => true,
            Self::NoReferences => true,
            Self::AuthRequired => true,
            Self::InvalidPassword => true,
            Self::OutOfJoinAttempts => true,
            Self::Euphoxide(euphoxide::Error::Tungstenite(tungstenite::Error::Http(response))) => {
                response.status() == StatusCode::NOT_FOUND
            }
            _ => false,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stopped => write!(f, "the instance was stopped manually"),
            Self::NoReferences => write!(f, "all references to the instance were dropped"),
            Self::AuthRequired => write!(f, "authentication required but no credentials found"),
            Self::InvalidPassword => write!(f, "authentication required but password is invalid"),
            Self::OutOfJoinAttempts => write!(f, "failed to join within attempt limit"),
            Self::Euphoxide(error) => write!(f, "{error}"),
        }
    }
}

impl From<euphoxide::Error> for Error {
    fn from(value: euphoxide::Error) -> Self {
        Self::Euphoxide(value)
    }
}

type Result<T> = result::Result<T, Error>;

enum Command {
    GetConn(oneshot::Sender<ClientConnHandle>),
    Stop,
}

/// Something happened.
#[derive(Debug)]
pub enum ClientEvent {
    /// The client has been started.
    Started,
    /// The client has started connecting to the room.
    Connecting,
    /// The client has established a connection to the room.
    Connected {
        /// The connection.
        conn: ClientConnHandle,
        /// The connection's state at the time of the event.
        state: State,
    },
    /// The client has joined the room and is ready to talk.
    Joined {
        /// The connection.
        conn: ClientConnHandle,
        /// The connection's state at the time of the event.
        state: State,
    },
    /// The client has received a packet from the server.
    Packet {
        /// The connection.
        conn: ClientConnHandle,
        /// The connection's state at the time of the event.
        state: State,
        /// The received packet.
        packet: ParsedPacket,
    },
    /// The client has disconnected from the room.
    ///
    /// For this event to fire, [`Self::Connected`] must have fired beforehand;
    /// [`Self::Connecting`] alone is not sufficient.
    Disconnected,
    /// The client has been stopped.
    Stopped,
}

struct ClientTask {
    id: usize,
    config: Arc<ClientConfig>,

    cmd_rx: mpsc::Receiver<Command>,
    event_tx: mpsc::Sender<(usize, ClientEvent)>,

    attempts: usize,
    never_joined: bool,
}

impl ClientTask {
    fn get_cookies(&self) -> Option<HeaderValue> {
        self.config
            .server
            .cookies
            .lock()
            .unwrap()
            .iter()
            .map(|c| c.stripped().to_string())
            .collect::<Vec<_>>()
            .join("; ")
            .try_into()
            .ok()
    }

    fn set_cookies(&mut self, cookies: &[HeaderValue]) {
        let mut guard = self.config.server.cookies.lock().unwrap();
        for cookie in cookies {
            if let Ok(cookie) = cookie.to_str()
                && let Ok(cookie) = Cookie::from_str(cookie)
            {
                guard.add(cookie);
            }
        }
    }

    async fn connect(&mut self) -> Result<ClientConn> {
        let (conn, cookies) = ClientConn::connect_with_config(
            &self.config.room,
            self.get_cookies(),
            &self.config.server.client,
        )
        .await?;

        self.set_cookies(&cookies);

        Ok(conn)
    }

    async fn on_joined(&mut self, conn: &ClientConn) {
        self.never_joined = false;

        let event = ClientEvent::Joined {
            conn: conn.handle(),
            state: conn.state().clone(),
        };
        let _ = self.event_tx.send((self.id, event)).await;
    }

    async fn on_packet(&mut self, conn: &mut ClientConn, packet: ParsedPacket) -> Result<()> {
        let event = ClientEvent::Packet {
            conn: conn.handle(),
            state: conn.state().clone(),
            packet: packet.clone(),
        };
        let _ = self.event_tx.send((self.id, event)).await;

        match packet.into_data()? {
            // Attempting to authenticate
            Data::BounceEvent(_) => {
                if let Some(password) = &self.config.password {
                    conn.send(Auth {
                        r#type: AuthOption::Passcode,
                        passcode: Some(password.clone()),
                    })
                    .await?;
                } else {
                    return Err(Error::AuthRequired);
                }
            }

            // Auth attempt failed :(
            Data::AuthReply(ev) if !ev.success => return Err(Error::InvalidPassword),

            // Just joined
            Data::SnapshotEvent(ev) => {
                if let Some(username) = &self.config.username
                    && (ev.nick.is_none() || self.config.force_username)
                {
                    conn.send(Nick {
                        name: username.clone(),
                    })
                    .await?;
                }

                // Maybe we should only count this as joining if we successfully
                // updated the nick instead of just sending a Nick command? And
                // maybe we should ensure that we're in the State::Joined state?
                // Both of these would probably complicate the code a lot. On
                // the other hand, InstanceEvent::Joined::state would contain
                // the actual nick after joining, which feels like the right
                // thing to do™. Probably not worth the increase in code
                // complexity though.

                self.on_joined(conn).await;
            }

            _ => {}
        }

        Ok(())
    }

    async fn on_cmd(&mut self, conn: &ClientConn, cmd: Command) -> Result<()> {
        match cmd {
            Command::GetConn(sender) => {
                let _ = sender.send(conn.handle());
                Ok(())
            }
            Command::Stop => Err(Error::Stopped),
        }
    }

    async fn run_once(&mut self) -> Result<()> {
        // If we try to connect too many times without managing to join at least
        // once, the room is probably not accessible for one reason or another
        // and the instance should stop.
        self.attempts += 1;
        if self.never_joined && self.attempts > self.config.server.join_attempts {
            return Err(Error::OutOfJoinAttempts);
        }

        let _ = self.event_tx.send((self.id, ClientEvent::Connecting)).await;

        let mut conn = match self.connect().await {
            Ok(conn) => conn,
            Err(err) => {
                // When we fail to connect, we want to wait a bit before
                // reconnecting in order not to spam the server. However, when
                // we are connected successfully and then disconnect for
                // whatever reason, we want to try to reconnect immediately. We
                // might, for example, be disconnected from the server because
                // we just logged in.
                tokio::time::sleep(self.config.server.reconnect_delay).await;
                Err(err)?
            }
        };

        let event = ClientEvent::Connected {
            conn: conn.handle(),
            state: conn.state().clone(),
        };
        let _ = self.event_tx.send((self.id, event)).await;

        let result = loop {
            let received = select! {
                r = conn.recv() => Ok(r?),
                r = self.cmd_rx.recv() => Err(r),
            };

            match received {
                // We received a packet
                Ok(None) => break Ok(()), // Connection closed
                Ok(Some(packet)) => self.on_packet(&mut conn, packet).await?,
                // We received a command
                Err(None) => break Err(Error::NoReferences),
                Err(Some(cmd)) => self.on_cmd(&conn, cmd).await?,
            };
        };

        let _ = self
            .event_tx
            .send((self.id, ClientEvent::Disconnected))
            .await;

        result
    }

    async fn run(mut self) {
        let _ = self.event_tx.send((self.id, ClientEvent::Started)).await;

        loop {
            if let Err(err) = self.run_once().await {
                warn!("instance {:?}: {err}", self.id);
                if err.is_fatal() {
                    break;
                }
            }
        }

        let _ = self.event_tx.send((self.id, ClientEvent::Stopped)).await;
    }
}

/// A persistent session in a room.
///
/// A [`Client`] represents a single session in a room that may span multiple
/// connections. It reconnects when it loses connection, authenticates when
/// required, and sets its nick upon (re-)joining.
///
/// While running, it emits events when something happens. The events can be
/// associated with the client using a unique ID (which you can just ignore if
/// you don't need it).
#[derive(Clone)]
pub struct Client {
    id: usize,
    config: Arc<ClientConfig>,
    cmd_tx: mpsc::Sender<Command>,
    start_time: Timestamp,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Instance")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Create a new client.
    ///
    /// The client will report events to the provided mpsc queue.
    pub fn new(
        id: usize,
        config: ClientConfig,
        event_tx: mpsc::Sender<(usize, ClientEvent)>,
    ) -> Self {
        let start_time = Timestamp::now();

        let config = Arc::new(config);

        let (cmd_tx, cmd_rx) = mpsc::channel(1);

        let task = ClientTask {
            id,
            config: config.clone(),
            attempts: 0,
            never_joined: false,
            cmd_rx,
            event_tx,
        };

        tokio::task::spawn(task.run());

        Self {
            id,
            config,
            cmd_tx,
            start_time,
        }
    }

    /// The client's unique ID.
    pub fn id(&self) -> usize {
        self.id
    }

    /// The client's config.
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// The time the client was created.
    pub fn start_time(&self) -> Timestamp {
        self.start_time
    }

    /// Whether the client has stopped.
    pub fn stopped(&self) -> bool {
        self.cmd_tx.is_closed()
    }

    /// Stop the client.
    pub async fn stop(&self) {
        let _ = self.cmd_tx.send(Command::Stop).await;
    }

    /// Get a handle to the client's connection, if it is currently connected.
    pub async fn handle(&self) -> Option<ClientConnHandle> {
        let (tx, rx) = oneshot::channel();
        let _ = self.cmd_tx.send(Command::GetConn(tx)).await;
        rx.await.ok()
    }
}

/////////////
// Builder //
/////////////

impl Client {
    /// Create a builder with the default [`ServerConfig`].
    pub fn builder(room: impl ToString) -> ClientBuilder<()> {
        Self::builder_for_server(ServerConfig::default(), room)
    }

    /// Create a builder for [`Client`]s.
    pub fn builder_for_server(server: ServerConfig, room: impl ToString) -> ClientBuilder<()> {
        ClientBuilder {
            base: (),
            config: ClientConfig::new(server, room.to_string()),
        }
    }
}

impl ClientBuilder<()> {
    /// Build a client.
    pub fn build(self, id: usize, event_tx: mpsc::Sender<(usize, ClientEvent)>) -> Client {
        Client::new(id, self.config, event_tx)
    }
}
