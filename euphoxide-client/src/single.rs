use std::{fmt, result, str::FromStr};

use cookie::Cookie;
use euphoxide::{
    api::{Auth, AuthOption, BounceEvent, Data, Nick, ParsedPacket},
    client::{
        conn::{ClientConn, ClientConnHandle},
        state::State,
    },
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

use crate::{ClientBuilder, ClientBuilderBase, ClientConfig, ServerConfig};

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

#[derive(Debug)]
pub enum ClientEvent {
    Started {
        id: usize,
    },
    Connecting {
        id: usize,
    },
    Connected {
        id: usize,
        conn: ClientConnHandle,
        state: State,
    },
    Joined {
        id: usize,
        conn: ClientConnHandle,
        state: State,
    },
    Packet {
        id: usize,
        conn: ClientConnHandle,
        state: State,
        packet: ParsedPacket,
    },
    Disconnected {
        id: usize,
    },
    Stopped {
        id: usize,
    },
}

impl ClientEvent {
    pub fn id(&self) -> usize {
        match self {
            Self::Started { id } => *id,
            Self::Connecting { id } => *id,
            Self::Connected { id, .. } => *id,
            Self::Joined { id, .. } => *id,
            Self::Packet { id, .. } => *id,
            Self::Disconnected { id } => *id,
            Self::Stopped { id } => *id,
        }
    }
}

struct ClientTask {
    id: usize,
    config: ClientConfig,

    cmd_rx: mpsc::Receiver<Command>,
    event_tx: mpsc::Sender<ClientEvent>,

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
            if let Ok(cookie) = cookie.to_str() {
                if let Ok(cookie) = Cookie::from_str(cookie) {
                    guard.add(cookie);
                }
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

        let _ = self
            .event_tx
            .send(ClientEvent::Joined {
                id: self.id,
                conn: conn.handle(),
                state: conn.state().clone(),
            })
            .await;
    }

    async fn on_packet(&mut self, conn: &mut ClientConn, packet: ParsedPacket) -> Result<()> {
        let _ = self
            .event_tx
            .send(ClientEvent::Packet {
                id: self.id,
                conn: conn.handle(),
                state: conn.state().clone(),
                packet: packet.clone(),
            })
            .await;

        match packet.into_data()? {
            // Attempting to authenticate
            Data::BounceEvent(BounceEvent {
                auth_options: Some(auth_options),
                ..
            }) if auth_options.contains(&AuthOption::Passcode) => {
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
                if let Some(username) = &self.config.username {
                    if ev.nick.is_none() || self.config.force_username {
                        conn.send(Nick {
                            name: username.clone(),
                        })
                        .await?;
                    }
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

        let _ = self
            .event_tx
            .send(ClientEvent::Connecting { id: self.id })
            .await;

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

        let _ = self
            .event_tx
            .send(ClientEvent::Connected {
                id: self.id,
                conn: conn.handle(),
                state: conn.state().clone(),
            })
            .await;

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
            .send(ClientEvent::Disconnected { id: self.id })
            .await;

        result
    }

    async fn run(mut self) {
        let _ = self
            .event_tx
            .send(ClientEvent::Started { id: self.id })
            .await;

        loop {
            if let Err(err) = self.run_once().await {
                warn!("instance {:?}: {err}", self.id);
                if err.is_fatal() {
                    break;
                }
            }
        }

        let _ = self
            .event_tx
            .send(ClientEvent::Stopped { id: self.id })
            .await;
    }
}

#[derive(Clone)]
pub struct Client {
    id: usize,
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
    pub fn new(id: usize, config: ClientConfig, event_tx: mpsc::Sender<ClientEvent>) -> Self {
        let start_time = Timestamp::now();

        let (cmd_tx, cmd_rx) = mpsc::channel(config.server.cmd_channel_bufsize);

        let task = ClientTask {
            id,
            config,
            attempts: 0,
            never_joined: false,
            cmd_rx,
            event_tx,
        };

        tokio::task::spawn(task.run());

        Self {
            id,
            cmd_tx,
            start_time,
        }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn start_time(&self) -> Timestamp {
        self.start_time
    }

    pub fn stopped(&self) -> bool {
        self.cmd_tx.is_closed()
    }

    pub async fn stop(&self) {
        let _ = self.cmd_tx.send(Command::Stop).await;
    }

    pub async fn handle(&self) -> Option<ClientConnHandle> {
        let (tx, rx) = oneshot::channel();
        let _ = self.cmd_tx.send(Command::GetConn(tx)).await;
        rx.await.ok()
    }
}

/////////////
// Builder //
/////////////

impl ClientBuilderBase<'static> for Client {
    type Base = ();
}

impl Client {
    pub fn builder(room: impl ToString) -> ClientBuilder<'static, Self> {
        Self::builder_for_server(ServerConfig::default(), room)
    }

    pub fn builder_for_server(
        server: ServerConfig,
        room: impl ToString,
    ) -> ClientBuilder<'static, Self> {
        ClientBuilder {
            base: (),
            config: ClientConfig::new(server, room.to_string()),
        }
    }
}

impl ClientBuilder<'static, Client> {
    pub fn build(self, id: usize, event_tx: mpsc::Sender<ClientEvent>) -> Client {
        Client::new(id, self.config, event_tx)
    }
}
