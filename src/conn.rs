//! Connection state modeling.

// TODO Catch errors differently when sending into mpsc/oneshot

use std::collections::HashMap;
use std::convert::Infallible;
use std::future::Future;
use std::time::Duration;
use std::{error, fmt};

use futures::channel::oneshot;
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::{select, task, time};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{header, HeaderValue};
use tokio_tungstenite::{tungstenite, MaybeTlsStream, WebSocketStream};

use crate::api::packet::{Command, Packet, ParsedPacket};
use crate::api::{
    BounceEvent, Data, HelloEvent, LoginReply, NickEvent, PersonalAccountView, Ping, PingReply,
    SessionId, SessionView, SnapshotEvent, Time, UserId,
};
use crate::replies::{self, PendingReply, Replies};

pub type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug)]
pub enum Error {
    ConnectionClosed,
    TimedOut,
    IncorrectReplyType,
    Euph(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionClosed => write!(f, "connection closed"),
            Self::TimedOut => write!(f, "packet timed out"),
            Self::IncorrectReplyType => write!(f, "incorrect reply type"),
            Self::Euph(error_msg) => write!(f, "{error_msg}"),
        }
    }
}

impl error::Error for Error {}

type InternalResult<T> = Result<T, Box<dyn error::Error>>;

#[derive(Debug)]
enum Event {
    Message(tungstenite::Message),
    SendCmd(Data, oneshot::Sender<PendingReply<ParsedPacket>>),
    SendRpl(Option<String>, Data),
    Status(oneshot::Sender<Status>),
    DoPings,
}

impl Event {
    fn send_cmd<C: Into<Data>>(cmd: C, rpl: oneshot::Sender<PendingReply<ParsedPacket>>) -> Self {
        Self::SendCmd(cmd.into(), rpl)
    }

    fn send_rpl<C: Into<Data>>(id: Option<String>, rpl: C) -> Self {
        Self::SendRpl(id, rpl.into())
    }
}

#[derive(Debug, Clone, Default)]
pub struct Joining {
    pub hello: Option<HelloEvent>,
    pub snapshot: Option<SnapshotEvent>,
    pub bounce: Option<BounceEvent>,
}

impl Joining {
    fn on_data(&mut self, data: &Data) -> InternalResult<()> {
        match data {
            Data::BounceEvent(p) => self.bounce = Some(p.clone()),
            Data::HelloEvent(p) => self.hello = Some(p.clone()),
            Data::SnapshotEvent(p) => self.snapshot = Some(p.clone()),
            Data::JoinEvent(_)
            | Data::NetworkEvent(_)
            | Data::NickEvent(_)
            | Data::EditMessageEvent(_)
            | Data::PartEvent(_)
            | Data::PmInitiateEvent(_)
            | Data::SendEvent(_) => return Err("unexpected packet type".into()),
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
pub enum Status {
    Joining(Joining),
    Joined(Joined),
}

struct State {
    ws_tx: SplitSink<WsStream, tungstenite::Message>,
    last_id: usize,
    replies: Replies<String, ParsedPacket>,

    packet_tx: mpsc::UnboundedSender<ParsedPacket>,

    // The server may send a pong frame with arbitrary payload unprompted at any
    // time (see RFC 6455 5.5.3). Because of this, we can't just remember the
    // last pong payload.
    ws_ping_counter: u64,
    last_ws_ping: Option<Vec<u8>>,
    last_ws_ping_replied_to: bool,

    last_euph_ping: Option<Time>,
    last_euph_pong: Option<Time>,

    status: Status,
}

impl State {
    async fn run(
        ws: WsStream,
        timeout: Duration,
        mut tx_canary: mpsc::UnboundedReceiver<Infallible>,
        rx_canary: oneshot::Receiver<Infallible>,
        event_tx: mpsc::UnboundedSender<Event>,
        mut event_rx: mpsc::UnboundedReceiver<Event>,
        packet_tx: mpsc::UnboundedSender<ParsedPacket>,
    ) {
        let (ws_tx, mut ws_rx) = ws.split();
        let mut state = Self {
            ws_tx,
            last_id: 0,
            replies: Replies::new(timeout),
            packet_tx,
            ws_ping_counter: 0,
            last_ws_ping: None,
            last_ws_ping_replied_to: false,
            last_euph_ping: None,
            last_euph_pong: None,
            status: Status::Joining(Joining::default()),
        };

        select! {
            _ = tx_canary.recv() => (),
            _ = rx_canary => (),
            _ = Self::listen(&mut ws_rx, &event_tx) => (),
            _ = Self::send_ping_events(&event_tx, timeout) => (),
            _ = state.handle_events(&event_tx, &mut event_rx) => (),
        }
    }

    async fn listen(
        ws_rx: &mut SplitStream<WsStream>,
        event_tx: &mpsc::UnboundedSender<Event>,
    ) -> InternalResult<()> {
        while let Some(msg) = ws_rx.next().await {
            event_tx.send(Event::Message(msg?))?;
        }
        Ok(())
    }

    async fn send_ping_events(
        event_tx: &mpsc::UnboundedSender<Event>,
        timeout: Duration,
    ) -> InternalResult<()> {
        loop {
            event_tx.send(Event::DoPings)?;
            time::sleep(timeout).await;
        }
    }

    async fn handle_events(
        &mut self,
        event_tx: &mpsc::UnboundedSender<Event>,
        event_rx: &mut mpsc::UnboundedReceiver<Event>,
    ) -> InternalResult<()> {
        while let Some(ev) = event_rx.recv().await {
            self.replies.purge();
            match ev {
                Event::Message(msg) => self.on_msg(msg, event_tx)?,
                Event::SendCmd(data, reply_tx) => self.on_send_cmd(data, reply_tx).await?,
                Event::SendRpl(id, data) => self.on_send_rpl(id, data).await?,
                Event::Status(reply_tx) => self.on_status(reply_tx),
                Event::DoPings => self.do_pings(event_tx).await?,
            }
        }
        Ok(())
    }

    fn on_msg(
        &mut self,
        msg: tungstenite::Message,
        event_tx: &mpsc::UnboundedSender<Event>,
    ) -> InternalResult<()> {
        match msg {
            tungstenite::Message::Text(t) => self.on_packet(serde_json::from_str(&t)?, event_tx)?,
            tungstenite::Message::Binary(_) => return Err("unexpected binary message".into()),
            tungstenite::Message::Ping(_) => {}
            tungstenite::Message::Pong(p) => {
                if self.last_ws_ping == Some(p) {
                    self.last_ws_ping_replied_to = true;
                }
            }
            tungstenite::Message::Close(_) => {}
            tungstenite::Message::Frame(_) => {}
        }
        Ok(())
    }

    fn on_packet(
        &mut self,
        packet: Packet,
        event_tx: &mpsc::UnboundedSender<Event>,
    ) -> InternalResult<()> {
        let packet = ParsedPacket::from_packet(packet)?;

        // Complete pending replies if the packet has an id
        if let Some(id) = &packet.id {
            self.replies.complete(id, packet.clone());
        }

        // Play a game of table tennis
        match &packet.content {
            Ok(Data::PingReply(p)) => self.last_euph_pong = p.time,
            Ok(Data::PingEvent(p)) => {
                let reply = PingReply { time: Some(p.time) };
                event_tx.send(Event::send_rpl(packet.id.clone(), reply))?;
            }
            _ => {}
        }

        // Update internal state
        if let Ok(data) = &packet.content {
            match &mut self.status {
                Status::Joining(joining) => {
                    joining.on_data(data)?;
                    if let Some(joined) = joining.joined() {
                        self.status = Status::Joined(joined);
                    }
                }
                Status::Joined(joined) => joined.on_data(data),
            }

            // The euphoria server doesn't always disconnect the client
            // when it would make sense to do so or when the API
            // specifies it should. This ensures we always disconnect
            // when it makes sense to do so.
            match data {
                Data::DisconnectEvent(_) => return Err("received disconnect-event".into()),
                Data::LoginEvent(_) => return Err("received login-event".into()),
                Data::LogoutEvent(_) => return Err("received logout-event".into()),
                Data::LoginReply(LoginReply { success: true, .. }) => {
                    return Err("received successful login-reply".into())
                }
                Data::LogoutReply(_) => return Err("received logout-reply".into()),
                _ => {}
            }
        }

        // Shovel packets into self.packet_tx
        self.packet_tx.send(packet)?;

        Ok(())
    }

    async fn on_send_cmd(
        &mut self,
        data: Data,
        reply_tx: oneshot::Sender<PendingReply<ParsedPacket>>,
    ) -> InternalResult<()> {
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
        self.ws_tx.send(msg).await?;

        let _ = reply_tx.send(self.replies.wait_for(id));

        Ok(())
    }

    async fn on_send_rpl(&mut self, id: Option<String>, data: Data) -> InternalResult<()> {
        let packet = ParsedPacket {
            id,
            r#type: data.packet_type(),
            content: Ok(data),
            throttled: None,
        }
        .into_packet()?;

        let msg = tungstenite::Message::Text(serde_json::to_string(&packet)?);
        self.ws_tx.send(msg).await?;

        Ok(())
    }

    fn on_status(&mut self, reply_tx: oneshot::Sender<Status>) {
        let _ = reply_tx.send(self.status.clone());
    }

    async fn do_pings(&mut self, event_tx: &mpsc::UnboundedSender<Event>) -> InternalResult<()> {
        // Check old ws ping
        if self.last_ws_ping.is_some() && !self.last_ws_ping_replied_to {
            return Err("server missed ws ping".into());
        }

        // Send new ws ping
        let ws_payload = self.ws_ping_counter.to_be_bytes().to_vec();
        self.ws_ping_counter = self.ws_ping_counter.wrapping_add(1);
        self.last_ws_ping = Some(ws_payload.clone());
        self.last_ws_ping_replied_to = false;
        self.ws_tx
            .send(tungstenite::Message::Ping(ws_payload))
            .await?;

        // Check old euph ping
        if self.last_euph_ping.is_some() && self.last_euph_ping != self.last_euph_pong {
            return Err("server missed euph ping".into());
        }

        // Send new euph ping
        let euph_payload = Time::now();
        self.last_euph_ping = Some(euph_payload);
        let (tx, _) = oneshot::channel();
        event_tx.send(Event::send_cmd(Ping { time: euph_payload }, tx))?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ConnTx {
    #[allow(dead_code)]
    canary: mpsc::UnboundedSender<Infallible>,
    event_tx: mpsc::UnboundedSender<Event>,
}

impl ConnTx {
    async fn finish_send<C>(
        rx: oneshot::Receiver<PendingReply<ParsedPacket>>,
    ) -> Result<C::Reply, Error>
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
                replies::Error::TimedOut => Error::TimedOut,
                replies::Error::Canceled => Error::ConnectionClosed,
            })?
            .content
            .map_err(Error::Euph)?;
        data.try_into().map_err(|_| Error::IncorrectReplyType)
    }

    /// Send a command to the server.
    ///
    /// Returns a future containing the server's reply. This future does not
    /// have to be awaited and can be safely ignored if you are not interested
    /// in the reply.
    ///
    /// This function may return before the command was sent. To ensure that it
    /// was sent, await the returned future first.
    ///
    /// When called multiple times, this function guarantees that the commands
    /// are sent in the order that the function is called.
    pub fn send<C>(&self, cmd: C) -> impl Future<Output = Result<C::Reply, Error>>
    where
        C: Command + Into<Data>,
        C::Reply: TryFrom<Data>,
    {
        let (tx, rx) = oneshot::channel();
        let _ = self.event_tx.send(Event::SendCmd(cmd.into(), tx));
        Self::finish_send::<C>(rx)
    }

    pub async fn status(&self) -> Result<Status, Error> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(Event::Status(tx))
            .map_err(|_| Error::ConnectionClosed)?;
        rx.await.map_err(|_| Error::ConnectionClosed)
    }
}

#[derive(Debug)]
pub struct ConnRx {
    #[allow(dead_code)]
    canary: oneshot::Sender<Infallible>,
    packet_rx: mpsc::UnboundedReceiver<ParsedPacket>,
}

impl ConnRx {
    pub async fn recv(&mut self) -> Option<ParsedPacket> {
        self.packet_rx.recv().await
    }
}

pub fn wrap(ws: WsStream, timeout: Duration) -> (ConnTx, ConnRx) {
    let (tx_canary_tx, tx_canary_rx) = mpsc::unbounded_channel();
    let (rx_canary_tx, rx_canary_rx) = oneshot::channel();
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let (packet_tx, packet_rx) = mpsc::unbounded_channel();

    task::spawn(State::run(
        ws,
        timeout,
        tx_canary_rx,
        rx_canary_rx,
        event_tx.clone(),
        event_rx,
        packet_tx,
    ));

    let tx = ConnTx {
        canary: tx_canary_tx,
        event_tx,
    };
    let rx = ConnRx {
        canary: rx_canary_tx,
        packet_rx,
    };
    (tx, rx)
}

pub async fn connect(
    domain: &str,
    room: &str,
    human: bool,
    cookies: Option<HeaderValue>,
    timeout: Duration,
) -> tungstenite::Result<(ConnTx, ConnRx, Vec<HeaderValue>)> {
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
    let (tx, rx) = wrap(ws, timeout);
    Ok((tx, rx, set_cookies))
}
