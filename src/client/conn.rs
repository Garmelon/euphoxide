//! Client-specific connection with a more expressive API.

use std::{future::Future, time::Duration};

use log::debug;
use tokio::{
    select,
    sync::{mpsc, oneshot},
};
use tokio_tungstenite::tungstenite::{
    client::IntoClientRequest,
    http::{header, HeaderValue},
};

use crate::{
    api::{Command, Data, ParsedPacket},
    conn::{Conn, ConnConfig, Side},
    replies::{self, PendingReply, Replies},
    Error, Result,
};

use super::state::State;

enum ConnCommand {
    SendCmd(Data, oneshot::Sender<Result<PendingReply<ParsedPacket>>>),
    GetState(oneshot::Sender<State>),
}

/// Configuration options for a [`ClientConn`].
#[derive(Debug, Clone)]
pub struct ClientConnConfig {
    /// The domain where the server is hosted.
    pub domain: String,
    /// Whether the client should present itself as a human to the server.
    ///
    /// This should only be set if the client is directly acting on behalf of a
    /// human, similar to the web client.
    pub human: bool,
    /// The size of the [`mpsc::channel`] for communication between
    /// [`ClientConn`] and [`ClientConnHandle`].
    pub channel_bufsize: usize,
    /// Timeout for opening a websocket connection.
    pub connect_timeout: Duration,
    /// Timeout for server replies when sending euphoria commands, i.e. packets
    /// implementing [`Command`].
    pub command_timeout: Duration,
    /// How long to wait in-between sending pings.
    ///
    /// See also [`ConnConfig::ping_interval`].
    pub ping_interval: Duration,
}

impl Default for ClientConnConfig {
    fn default() -> Self {
        Self {
            domain: "euphoria.leet.nu".to_string(),
            human: false,
            channel_bufsize: 10,
            connect_timeout: Duration::from_secs(10),
            command_timeout: Duration::from_secs(30),
            ping_interval: Duration::from_secs(30),
        }
    }
}

/// A client connection to an euphoria server.
///
/// This struct is a wrapper around [`Conn`] with a more client-centric API. It
/// tracks the connection state, including room information sent by the server.
/// It also provides [`ClientConnHandle`], which can be used to asynchronously
/// send commands and await their replies.
pub struct ClientConn {
    rx: mpsc::Receiver<ConnCommand>,
    tx: mpsc::Sender<ConnCommand>,

    conn: Conn,
    state: State,

    last_id: usize,
    replies: Replies<String, ParsedPacket>,
}

impl ClientConn {
    /// Retrieve the current [`State`] of the connection.
    pub fn state(&self) -> &State {
        &self.state
    }

    /// Create a new handle for this connection.
    pub fn handle(&self) -> ClientConnHandle {
        ClientConnHandle {
            tx: self.tx.clone(),
        }
    }

    /// Start closing the connection.
    ///
    /// To finish closing the connection gracefully, continue calling
    /// [`Self::recv`] until it returns [`None`].
    pub async fn close(&mut self) -> Result<()> {
        self.conn.close().await
    }

    /// Receive a [`ParsedPacket`] over the connection.
    ///
    /// This method also maintains the connection by listening and responding to
    /// pings as well as managing [`ClientConnHandle`]s. Thus, it must be called
    /// regularly.
    ///
    /// Returns [`None`] if the connection is closed.
    pub async fn recv(&mut self) -> Result<Option<ParsedPacket>> {
        loop {
            self.replies.purge();

            // There's always at least one tx end (self.tx), so self.rx.recv()
            // should never return None.
            let packet = select! {
                packet = self.conn.recv() => packet?,
                Some(cmd) = self.rx.recv() => {
                    self.on_cmd(cmd).await;
                    continue;
                },
            };

            if let Some(packet) = &packet {
                self.on_packet(packet);
            }

            break Ok(packet);
        }
    }

    /// Send a packet over the connection.
    ///
    /// A packet id is automatically generated and returned. When the server
    /// replies to the packet, it will use this id as its [`ParsedPacket::id`].
    pub async fn send(&mut self, data: impl Into<Data>) -> Result<String> {
        // Overkill of universe-heat-death-like proportions
        self.last_id = self.last_id.wrapping_add(1);
        let id = self.last_id.to_string();

        self.conn
            .send(ParsedPacket::from_data(Some(id.clone()), data.into()))
            .await?;

        Ok(id)
    }

    fn on_packet(&mut self, packet: &ParsedPacket) {
        if let Ok(data) = &packet.content {
            self.state.on_data(data);
        }

        if let Some(id) = &packet.id {
            let id = id.clone();
            self.replies.complete(&id, packet.clone());
        }
    }

    async fn on_cmd(&mut self, cmd: ConnCommand) {
        match cmd {
            ConnCommand::SendCmd(data, sender) => {
                let result = self.send(data).await.map(|id| self.replies.wait_for(id));
                let _ = sender.send(result);
            }
            ConnCommand::GetState(sender) => {
                let _ = sender.send(self.state.clone());
            }
        }
    }

    /// Connect to a room.
    ///
    /// See [`Self::connect_with_config`] for more details.
    pub async fn connect(
        room: &str,
        cookies: Option<HeaderValue>,
    ) -> Result<(Self, Vec<HeaderValue>)> {
        Self::connect_with_config(room, cookies, &ClientConnConfig::default()).await
    }

    /// Connect to a room with a specific configuration.
    ///
    /// Cookies to be sent to the server can be specified as a [`HeaderValue`]
    /// in the format of a [`Cookie` request header][0]. If the connection
    /// attempt was successful, cookies set by the server will be returned
    /// alongside the connection itself as one [`HeaderValue`] per [`Set-Cookie`
    /// response header][1].
    ///
    /// The tasks of cookie parsing and storage are not handled by this library.
    ///
    /// [0]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Cookie
    /// [1]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Set-Cookie
    pub async fn connect_with_config(
        room: &str,
        cookies: Option<HeaderValue>,
        config: &ClientConnConfig,
    ) -> Result<(Self, Vec<HeaderValue>)> {
        // Prepare URL
        let human = if config.human { "?h=1" } else { "" };
        let uri = format!("wss://{}/room/{room}/ws{human}", config.domain);
        debug!("Connecting to {uri} with cookies: {cookies:?}");

        // Prepare request
        let mut request = uri.into_client_request().expect("valid request");
        if let Some(cookies) = cookies {
            request.headers_mut().append(header::COOKIE, cookies);
        }

        // Connect to server
        let (ws, response) = tokio::time::timeout(
            config.connect_timeout,
            tokio_tungstenite::connect_async(request),
        )
        .await
        .map_err(|_| Error::ConnectionTimeout)??;

        // Extract response cookies
        let (mut parts, _) = response.into_parts();
        let cookies_set = match parts.headers.entry(header::SET_COOKIE) {
            header::Entry::Occupied(entry) => entry.remove_entry_mult().1.collect(),
            header::Entry::Vacant(_) => vec![],
        };
        debug!("Received cookies {cookies_set:?}");

        // Prepare EuphConn
        let conn_config = ConnConfig {
            ping_interval: config.ping_interval,
        };
        let conn = Conn::wrap_with_config(ws, Side::Client, conn_config);

        // Prepare client
        let (tx, rx) = mpsc::channel(config.channel_bufsize);
        let client = Self {
            rx,
            tx,
            conn,
            state: State::new(),
            last_id: 0,
            replies: Replies::new(config.command_timeout),
        };

        Ok((client, cookies_set))
    }
}

/// Asynchronous access to a [`ClientConn`].
///
/// Handle methods are only processed while [`ClientConn::recv`] is being
/// called. They may return before they were processed by the associated
/// [`ClientConn`], or they may block until processed. Methods are processed in
/// the order they are called.
///
/// The handle is cheap to clone.
#[derive(Debug, Clone)]
pub struct ClientConnHandle {
    tx: mpsc::Sender<ConnCommand>,
}

impl ClientConnHandle {
    /// Send a command to the server.
    ///
    /// When awaited, returns either an error if something went wrong while
    /// sending the command, or a second future with the server's reply (the
    /// *reply future*).
    ///
    /// When awaited, the *reply future* returns either an error if something
    /// was wrong with the reply, or the data returned by the server. The *reply
    /// future* can be safely ignored and doesn't have to be awaited.
    pub async fn send<C>(&self, cmd: C) -> Result<impl Future<Output = Result<C::Reply>>>
    where
        C: Command + Into<Data>,
        C::Reply: TryFrom<Data>,
    {
        let (tx, rx) = oneshot::channel();

        self.tx
            .send(ConnCommand::SendCmd(cmd.into(), tx))
            .await
            .map_err(|_| Error::ConnectionClosed)?;

        Ok(async {
            let data = rx
                .await
                .map_err(|_| Error::ConnectionClosed)??
                .get()
                .await
                .map_err(|err| match err {
                    replies::Error::TimedOut => Error::CommandTimeout,
                    replies::Error::Canceled => Error::ConnectionClosed,
                })?
                .content
                .map_err(Error::Euph)?;

            let ptype = data.packet_type();
            data.try_into()
                .map_err(|_| Error::ReceivedUnexpectedPacket(ptype))
        })
    }

    /// Send a command to the server without waiting for a reply.
    ///
    /// This method is equivalent to calling and awaiting [`Self::send`] but
    /// ignoring the *reply future*. The reason it exists is that clippy gets
    /// really annoying when you try to ignore a future (which is usually the
    /// right call).
    pub async fn send_only<C>(&self, cmd: C) -> Result<()>
    where
        C: Command + Into<Data>,
        C::Reply: TryFrom<Data>,
    {
        let _ignore = self.send(cmd).await?;
        Ok(())
    }

    /// Retrieve the current connection [`State`].
    pub async fn state(&self) -> Result<State> {
        let (tx, rx) = oneshot::channel();

        self.tx
            .send(ConnCommand::GetState(tx))
            .await
            .map_err(|_| Error::ConnectionClosed)?;

        rx.await.map_err(|_| Error::ConnectionClosed)
    }
}
