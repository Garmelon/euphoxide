//! Connection state modeling.

use std::collections::HashMap;
use std::convert::Infallible;
use std::future::Future;
use std::time::{Duration, Instant};
use std::{error, fmt, result};

use ::time::OffsetDateTime;
use futures_util::SinkExt;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio::{select, time};
use tokio_stream::StreamExt;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{header, HeaderValue};
use tokio_tungstenite::{tungstenite, MaybeTlsStream, WebSocketStream};

use crate::api::packet::{Command, ParsedPacket};
use crate::api::{
    BounceEvent, Data, HelloEvent, LoginReply, NickEvent, PersonalAccountView, Ping, PingReply,
    SessionId, SessionView, SnapshotEvent, Time, UserId,
};
use crate::replies::{self, PendingReply, Replies};

pub type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug)]
pub enum Error {
    /// The connection is now closed.
    ConnectionClosed,
    /// The server didn't reply to one of our commands in time.
    CommandTimedOut,
    /// The server did something that violated the api specification.
    ProtocolViolation(&'static str),
    /// An error returned by the euphoria server.
    Euph(String),

    Tungstenite(tungstenite::Error),
    SerdeJson(serde_json::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionClosed => write!(f, "connection closed"),
            Self::CommandTimedOut => write!(f, "server did not reply to command in time"),
            Self::ProtocolViolation(msg) => write!(f, "{msg}"),
            Self::Euph(msg) => write!(f, "{msg}"),
            Self::Tungstenite(err) => write!(f, "{err}"),
            Self::SerdeJson(err) => write!(f, "{err}"),
        }
    }
}

impl From<tungstenite::Error> for Error {
    fn from(err: tungstenite::Error) -> Self {
        Self::Tungstenite(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::SerdeJson(err)
    }
}

impl error::Error for Error {}

type Result<T> = result::Result<T, Error>;

#[derive(Debug, Clone, Default)]
pub struct Joining {
    pub hello: Option<HelloEvent>,
    pub snapshot: Option<SnapshotEvent>,
    pub bounce: Option<BounceEvent>,
}

impl Joining {
    fn on_data(&mut self, data: &Data) -> Result<()> {
        match data {
            Data::BounceEvent(p) => self.bounce = Some(p.clone()),
            Data::HelloEvent(p) => self.hello = Some(p.clone()),
            Data::SnapshotEvent(p) => self.snapshot = Some(p.clone()),
            // TODO Check and maybe expand list of unexpected packet types
            Data::JoinEvent(_)
            | Data::NetworkEvent(_)
            | Data::NickEvent(_)
            | Data::EditMessageEvent(_)
            | Data::PartEvent(_)
            | Data::PmInitiateEvent(_)
            | Data::SendEvent(_) => return Err(Error::ProtocolViolation("unexpected packet type")),
            _ => {}
        }
        Ok(())
    }

    fn joined(&self) -> Option<Joined> {
        if let (Some(hello), Some(snapshot)) = (&self.hello, &self.snapshot) {
            let mut session = hello.session.clone();
            if let Some(nick) = &snapshot.nick {
                session.name = nick.clone();
            }
            let listing = snapshot
                .listing
                .iter()
                .cloned()
                .map(|s| (s.session_id.clone(), SessionInfo::Full(s)))
                .collect::<HashMap<_, _>>();
            Some(Joined {
                session,
                account: hello.account.clone(),
                listing,
            })
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub enum SessionInfo {
    Full(SessionView),
    Partial(NickEvent),
}

impl SessionInfo {
    pub fn id(&self) -> &UserId {
        match self {
            Self::Full(sess) => &sess.id,
            Self::Partial(nick) => &nick.id,
        }
    }

    pub fn session_id(&self) -> &SessionId {
        match self {
            Self::Full(sess) => &sess.session_id,
            Self::Partial(nick) => &nick.session_id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Full(sess) => &sess.name,
            Self::Partial(nick) => &nick.to,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Joined {
    pub session: SessionView,
    pub account: Option<PersonalAccountView>,
    pub listing: HashMap<SessionId, SessionInfo>,
}

impl Joined {
    fn on_data(&mut self, data: &Data) {
        match data {
            Data::JoinEvent(p) => {
                self.listing
                    .insert(p.0.session_id.clone(), SessionInfo::Full(p.0.clone()));
            }
            Data::SendEvent(p) => {
                self.listing.insert(
                    p.0.sender.session_id.clone(),
                    SessionInfo::Full(p.0.sender.clone()),
                );
            }
            Data::PartEvent(p) => {
                self.listing.remove(&p.0.session_id);
            }
            Data::NetworkEvent(p) => {
                if p.r#type == "partition" {
                    self.listing.retain(|_, s| match s {
                        SessionInfo::Full(s) => {
                            s.server_id != p.server_id && s.server_era != p.server_era
                        }
                        // We can't know if the session was disconnected by the
                        // partition or not, so we're erring on the side of
                        // caution and assuming they were kicked. If we're
                        // wrong, we'll re-add the session as soon as it
                        // performs another visible action.
                        //
                        // If we always kept such sessions, we might keep
                        // disconnected ones indefinitely, thereby keeping them
                        // from moving on, instead forever tethering them to the
                        // digital realm.
                        SessionInfo::Partial(_) => false,
                    });
                }
            }
            Data::NickEvent(p) => {
                self.listing
                    .entry(p.session_id.clone())
                    .and_modify(|s| match s {
                        SessionInfo::Full(session) => session.name = p.to.clone(),
                        SessionInfo::Partial(_) => *s = SessionInfo::Partial(p.clone()),
                    })
                    .or_insert_with(|| SessionInfo::Partial(p.clone()));
            }
            Data::NickReply(p) => {
                assert_eq!(self.session.id, p.id);
                self.session.name = p.to.clone();
            }
            // The who reply is broken and can't be trusted right now, so we'll
            // not even look at it.
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum State {
    Joining(Joining),
    Joined(Joined),
}

impl State {
    pub fn into_joining(self) -> Option<Joining> {
        match self {
            Self::Joining(joining) => Some(joining),
            Self::Joined(_) => None,
        }
    }

    pub fn into_joined(self) -> Option<Joined> {
        match self {
            Self::Joining(_) => None,
            Self::Joined(joined) => Some(joined),
        }
    }

    pub fn joining(&self) -> Option<&Joining> {
        match self {
            Self::Joining(joining) => Some(joining),
            Self::Joined(_) => None,
        }
    }

    pub fn joined(&self) -> Option<&Joined> {
        match self {
            Self::Joining(_) => None,
            Self::Joined(joined) => Some(joined),
        }
    }
}

#[allow(clippy::large_enum_variant)]
enum ConnCommand {
    SendCmd(Data, oneshot::Sender<PendingReply<ParsedPacket>>),
    GetState(oneshot::Sender<State>),
}

#[derive(Debug, Clone)]
pub struct ConnTx {
    cmd_tx: mpsc::UnboundedSender<ConnCommand>,
}

impl ConnTx {
    /// The async part of sending a command.
    ///
    /// This is split into a separate function so that [`Self::send`] can be
    /// fully synchronous (you can safely throw away the returned future) while
    /// still guaranteeing that the packet was sent.
    async fn finish_send<C>(rx: oneshot::Receiver<PendingReply<ParsedPacket>>) -> Result<C::Reply>
    where
        C: Command,
        C::Reply: TryFrom<Data>,
    {
        let pending_reply = rx
            .await
            // This should only happen if something goes wrong during encoding
            // of the packet or while sending it through the websocket. Assuming
            // the first doesn't happen, the connection is probably closed.
            .map_err(|_| Error::ConnectionClosed)?;

        let data = pending_reply
            .get()
            .await
            .map_err(|e| match e {
                replies::Error::TimedOut => Error::CommandTimedOut,
                replies::Error::Canceled => Error::ConnectionClosed,
            })?
            .content
            .map_err(Error::Euph)?;

        data.try_into()
            .map_err(|_| Error::ProtocolViolation("incorrect command reply type"))
    }

    /// Send a command to the server.
    ///
    /// Returns a future containing the server's reply. This future does not
    /// have to be awaited and can be safely ignored if you are not interested
    /// in the reply.
    ///
    /// This function may return before the command was sent. To ensure that it
    /// was sent before doing something else, await the returned future first.
    ///
    /// When called multiple times, this function guarantees that the commands
    /// are sent in the order that the function is called.
    pub fn send<C>(&self, cmd: C) -> impl Future<Output = Result<C::Reply>>
    where
        C: Command + Into<Data>,
        C::Reply: TryFrom<Data>,
    {
        let (tx, rx) = oneshot::channel();
        let _ = self.cmd_tx.send(ConnCommand::SendCmd(cmd.into(), tx));
        Self::finish_send::<C>(rx)
    }

    pub async fn state(&self) -> Result<State> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(ConnCommand::GetState(tx))
            .map_err(|_| Error::ConnectionClosed)?;
        rx.await.map_err(|_| Error::ConnectionClosed)
    }
}

#[derive(Debug)]
pub struct Conn {
    ws: WsStream,
    last_id: usize,
    replies: Replies<String, ParsedPacket>,

    conn_tx: ConnTx,
    cmd_rx: mpsc::UnboundedReceiver<ConnCommand>,

    // The websocket server may send a pong frame with arbitrary payload
    // unprompted at any time (see RFC 6455 5.5.3). Because of this, we can't
    // just remember the last pong payload.
    last_ping: Instant,
    last_ws_ping_payload: Option<Vec<u8>>,
    last_ws_ping_replied_to: bool,
    last_euph_ping_payload: Option<Time>,
    last_euph_ping_replied_to: bool,

    state: State,
}

enum ConnEvent {
    Ws(Option<tungstenite::Result<tungstenite::Message>>),
    Cmd(Option<ConnCommand>),
    Ping,
}

impl Conn {
    pub fn tx(&self) -> &ConnTx {
        &self.conn_tx
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub async fn recv(&mut self) -> Result<ParsedPacket> {
        loop {
            self.replies.purge();
            let timeout = self.replies.timeout();

            // All of these functions are cancel-safe.
            let event = select! {
                msg = self.ws.next() => ConnEvent::Ws(msg),
                cmd = self.cmd_rx.recv() => ConnEvent::Cmd(cmd),
                _ = Self::await_next_ping(self.last_ping, timeout) => ConnEvent::Ping,
            };

            match event {
                ConnEvent::Ws(msg) => {
                    if let Some(packet) = self.on_ws(msg).await? {
                        break Ok(packet);
                    }
                }
                ConnEvent::Cmd(Some(cmd)) => self.on_cmd(cmd).await?,
                ConnEvent::Cmd(None) => unreachable!("self contains a ConnTx"),
                ConnEvent::Ping => self.on_ping().await?,
            }
        }
    }

    async fn on_ws(
        &mut self,
        msg: Option<tungstenite::Result<tungstenite::Message>>,
    ) -> Result<Option<ParsedPacket>> {
        let msg = msg.ok_or(Error::ConnectionClosed)??;
        match msg {
            tungstenite::Message::Text(text) => {
                let packet = ParsedPacket::from_packet(serde_json::from_str(&text)?)?;
                self.on_packet(&packet).await?;
                return Ok(Some(packet));
            }
            tungstenite::Message::Binary(_) => {
                return Err(Error::ProtocolViolation("unexpected binary ws message"));
            }
            tungstenite::Message::Ping(_) => {}
            tungstenite::Message::Pong(payload) => {
                if self.last_ws_ping_payload == Some(payload) {
                    self.last_ws_ping_replied_to = true;
                }
            }
            tungstenite::Message::Close(_) => {}
            tungstenite::Message::Frame(_) => {}
        }
        Ok(None)
    }

    async fn on_packet(&mut self, packet: &ParsedPacket) -> Result<()> {
        // Complete pending replies if the packet has an id
        if let Some(id) = &packet.id {
            self.replies.complete(id, packet.clone());
        }

        match &packet.content {
            Ok(data) => self.on_data(&packet.id, data).await,
            Err(msg) => Err(Error::Euph(msg.clone())),
        }
    }

    async fn on_data(&mut self, id: &Option<String>, data: &Data) -> Result<()> {
        // Play a game of table tennis
        match data {
            Data::PingReply(p) => {
                if self.last_euph_ping_payload.is_some() && self.last_euph_ping_payload == p.time {
                    self.last_euph_ping_replied_to = true;
                }
            }
            Data::PingEvent(p) => {
                let reply = PingReply { time: Some(p.time) };
                self.send_rpl(id.clone(), reply.into()).await?;
            }
            _ => {}
        }

        // Update internal state
        match &mut self.state {
            State::Joining(joining) => {
                joining.on_data(data)?;
                if let Some(joined) = joining.joined() {
                    self.state = State::Joined(joined);
                }
            }
            State::Joined(joined) => joined.on_data(data),
        }

        // The euphoria server doesn't always disconnect the client when it
        // would make sense to do so or when the API specifies it should. This
        // ensures we always disconnect when it makes sense to do so.
        if matches!(
            data,
            Data::DisconnectEvent(_)
                | Data::LoginEvent(_)
                | Data::LogoutEvent(_)
                | Data::LoginReply(LoginReply { success: true, .. })
                | Data::LogoutReply(_)
        ) {
            self.disconnect().await?;
        }

        Ok(())
    }

    async fn on_cmd(&mut self, cmd: ConnCommand) -> Result<()> {
        match cmd {
            ConnCommand::SendCmd(data, reply_tx) => self.send_cmd(data, reply_tx).await?,
            ConnCommand::GetState(reply_tx) => {
                let _ = reply_tx.send(self.state.clone());
            }
        }
        Ok(())
    }

    async fn await_next_ping(last_ping: Instant, timeout: Duration) {
        let since_last_ping = last_ping.elapsed();
        if let Some(remaining) = timeout.checked_sub(since_last_ping) {
            time::sleep(remaining).await;
        }
    }

    async fn on_ping(&mut self) -> Result<()> {
        // Check previous pings
        if self.last_ws_ping_payload.is_some() && !self.last_ws_ping_replied_to {
            self.disconnect().await?;
        }
        if self.last_euph_ping_payload.is_some() && !self.last_euph_ping_replied_to {
            self.disconnect().await?;
        }

        let now = OffsetDateTime::now_utc();

        // Send new ws ping
        let ws_payload = now.unix_timestamp_nanos().to_be_bytes().to_vec();
        self.last_ws_ping_payload = Some(ws_payload.clone());
        self.ws.send(tungstenite::Message::Ping(ws_payload)).await?;

        // Send new euph ping
        let euph_payload = Time::new(now);
        self.last_euph_ping_payload = Some(euph_payload);
        let (tx, _) = oneshot::channel();
        self.send_cmd(Ping { time: euph_payload }.into(), tx)
            .await?;

        self.last_ping = Instant::now();

        Ok(())
    }

    async fn send_cmd(
        &mut self,
        data: Data,
        reply_tx: oneshot::Sender<PendingReply<ParsedPacket>>,
    ) -> Result<()> {
        // Overkill of universe-heat-death-like proportions
        self.last_id = self.last_id.wrapping_add(1);
        let id = format!("{}", self.last_id);

        let packet = ParsedPacket {
            id: Some(id.clone()),
            r#type: data.packet_type(),
            content: Ok(data),
            throttled: None,
        }
        .into_packet()?;

        let msg = tungstenite::Message::Text(serde_json::to_string(&packet)?);
        self.ws.send(msg).await?;

        let _ = reply_tx.send(self.replies.wait_for(id));

        Ok(())
    }

    async fn send_rpl(&mut self, id: Option<String>, data: Data) -> Result<()> {
        let packet = ParsedPacket {
            id,
            r#type: data.packet_type(),
            content: Ok(data),
            throttled: None,
        }
        .into_packet()?;

        let msg = tungstenite::Message::Text(serde_json::to_string(&packet)?);
        self.ws.send(msg).await?;

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<Infallible> {
        let _ = self.ws.close(None).await;
        Err(Error::ConnectionClosed)
    }

    pub fn wrap(ws: WsStream, timeout: Duration) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        Self {
            ws,
            last_id: 0,
            replies: Replies::new(timeout),

            conn_tx: ConnTx { cmd_tx },
            cmd_rx,

            last_ping: Instant::now(), // Wait a bit before first pings
            last_ws_ping_payload: None,
            last_ws_ping_replied_to: false,
            last_euph_ping_payload: None,
            last_euph_ping_replied_to: false,

            state: State::Joining(Joining::default()),
        }
    }

    pub async fn connect(
        domain: &str,
        room: &str,
        human: bool,
        cookies: Option<HeaderValue>,
        timeout: Duration,
    ) -> tungstenite::Result<(Self, Vec<HeaderValue>)> {
        let human = if human { "?h=1" } else { "" };
        let uri = format!("wss://{domain}/room/{room}/ws{human}");
        let mut request = uri.into_client_request().expect("valid request");
        if let Some(cookies) = cookies {
            request.headers_mut().append(header::COOKIE, cookies);
        }

        let (ws, response) = tokio_tungstenite::connect_async(request).await?;
        let (mut parts, _) = response.into_parts();
        let set_cookies = match parts.headers.entry(header::SET_COOKIE) {
            header::Entry::Occupied(entry) => entry.remove_entry_mult().1.collect(),
            header::Entry::Vacant(_) => vec![],
        };
        let rx = Self::wrap(ws, timeout);
        Ok((rx, set_cookies))
    }
}
