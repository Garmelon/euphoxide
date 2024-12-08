//! Basic connection between client and server.

use std::{fmt, time::Duration};

use futures_util::{SinkExt, StreamExt};
use jiff::Timestamp;
use log::debug;
use tokio::{
    net::TcpStream,
    select,
    time::{self, Instant},
};
use tokio_tungstenite::{
    tungstenite::{client::IntoClientRequest, handshake::client::Response, Message},
    MaybeTlsStream, WebSocketStream,
};

use crate::{
    api::{Data, Packet, PacketType, ParsedPacket, Ping, PingEvent, PingReply, Time},
    Error, Result,
};

/// Which side of the connection we're on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// We're the client and are talking to a server.
    Client,
    /// We're the server and are talking to a client.
    Server,
}

/// Configuration options for a [`Conn`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnConfig {
    /// How long to wait in-between pings.
    pub ping_interval: Duration,
}

impl Default for ConnConfig {
    fn default() -> Self {
        Self {
            ping_interval: Duration::from_secs(30),
        }
    }
}

/// A basic connection between a client and a server.
///
/// The connection can be used both from a server's and from a client's
/// perspective. In both cases, it performs regular websocket *and* euphoria
/// pings and terminates the connection if the other side does not reply before
/// the next ping is sent.
pub struct Conn {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    side: Side,
    config: ConnConfig,

    // The websocket server may send a pong frame with arbitrary payload
    // unprompted at any time (see RFC 6455 5.5.3). Because of this, we can't
    // just remember the last pong payload.
    last_ping: Instant,
    last_ws_ping_payload: Option<Vec<u8>>,
    last_ws_ping_replied_to: bool,
    last_euph_ping_payload: Option<Time>,
    last_euph_ping_replied_to: bool,
}

impl Conn {
    /// The connection's side.
    pub fn side(&self) -> Side {
        self.side
    }

    /// The connection's config.
    pub fn config(&self) -> &ConnConfig {
        &self.config
    }

    /// Connect to a given URL.
    pub async fn connect<R>(request: R) -> Result<(Self, Response)>
    where
        R: IntoClientRequest + Unpin,
    {
        Self::connect_with_config(request, ConnConfig::default()).await
    }

    /// Connect to a given URL with a specific configuration.
    pub async fn connect_with_config<R>(request: R, config: ConnConfig) -> Result<(Self, Response)>
    where
        R: IntoClientRequest + Unpin,
    {
        let (ws, response) = tokio_tungstenite::connect_async(request).await?;
        let conn = Self::wrap_with_config(ws, Side::Client, config);
        Ok((conn, response))
    }

    /// Wrap an existing websocket stream.
    pub fn wrap(ws: WebSocketStream<MaybeTlsStream<TcpStream>>, side: Side) -> Self {
        Self::wrap_with_config(ws, side, ConnConfig::default())
    }

    /// Wrap an existing websocket stream with a specific configuration.
    pub fn wrap_with_config(
        ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
        side: Side,
        config: ConnConfig,
    ) -> Self {
        Self {
            ws,
            side,
            config,
            last_ping: Instant::now(),
            last_ws_ping_payload: None,
            last_ws_ping_replied_to: false,
            last_euph_ping_payload: None,
            last_euph_ping_replied_to: false,
        }
    }

    /// Close the connection gracefully.
    pub async fn close(&mut self) -> Result<()> {
        self.ws.close(None).await?;
        Ok(())
    }

    /// Send a [`Packet`] over the connection.
    pub async fn send_raw(&mut self, packet: &Packet) -> Result<()> {
        debug!(target: "euphoxide::conn::full", "Sending {packet:?}");
        let text = serde_json::to_string(&packet).map_err(Error::MalformedPacket)?;
        self.ws.send(Message::Text(text)).await?;
        Ok(())
    }

    /// Send a [`ParsedPacket`] over the connection.
    pub async fn send(&mut self, packet: ParsedPacket) -> Result<()> {
        let packet = packet.into_packet().map_err(Error::MalformedPacket)?;
        self.send_raw(&packet).await
    }

    /// Receive a [`Packet`] over the connection.
    ///
    /// This method also listens for and sends pings in regular intervals as
    /// specified by [`ConnConfig::ping_interval`]. Thus, this method must be
    /// called regularly.
    ///
    /// Returns [`None`] if the connection is closed.
    pub async fn recv_raw(&mut self) -> Result<Option<Packet>> {
        loop {
            let next_ping = self.last_ping + self.config.ping_interval;

            let result = select! {
                _ = time::sleep_until(next_ping) => None,
                r = self.ws.next() => Some(r),
            };

            match result {
                None => self.check_and_send_pings().await?,
                Some(None) => break Ok(None),
                Some(Some(result)) => {
                    if let Some(packet) = self.on_message(result?).await? {
                        debug!(target: "euphoxide::conn::full", "Received {packet:?}");
                        break Ok(Some(packet));
                    }
                }
            }
        }
    }

    /// Receive a [`ParsedPacket`] over the connection.
    ///
    /// For more details, see [`Self::recv_raw`].
    pub async fn recv(&mut self) -> Result<Option<ParsedPacket>> {
        let Some(packet) = self.recv_raw().await? else {
            return Ok(None);
        };

        let packet = ParsedPacket::from_packet(packet).map_err(Error::ReceivedMalformedPacket)?;
        Ok(Some(packet))
    }

    async fn check_and_send_pings(&mut self) -> Result<()> {
        debug!("Checking ping replies and sending new pings");

        // Check previous ws ping
        if self.last_ws_ping_payload.is_some() && !self.last_ws_ping_replied_to {
            debug!("No response to websocket ping, disconnecting");
            self.close().await?;
            return Err(Error::PingTimeout);
        }

        // Check previous euph ping
        if self.last_euph_ping_payload.is_some() && !self.last_euph_ping_replied_to {
            debug!("No response to euph ping, disconnecting");
            self.close().await?;
            return Err(Error::PingTimeout);
        }

        let now = Timestamp::now();

        // Send new ws ping
        let ws_payload = now.as_millisecond().to_be_bytes().to_vec();
        self.last_ws_ping_payload = Some(ws_payload.clone());
        self.last_ws_ping_replied_to = false;
        self.ws.send(Message::Ping(ws_payload)).await?;

        // Send new euph ping
        let euph_payload = Time::from_timestamp(now);
        self.last_euph_ping_payload = Some(euph_payload);
        self.last_euph_ping_replied_to = false;
        let data: Data = match self.side {
            Side::Client => Ping { time: euph_payload }.into(),
            Side::Server => PingEvent {
                time: euph_payload,
                next: Time::from_timestamp(now + self.config.ping_interval),
            }
            .into(),
        };
        self.send(ParsedPacket::from_data(None, data)).await?;

        self.last_ping = Instant::now();

        Ok(())
    }

    async fn on_message(&mut self, message: Message) -> Result<Option<Packet>> {
        match message {
            Message::Pong(payload) => {
                if self.last_ws_ping_payload == Some(payload) {
                    debug!("Received valid ws pong");
                    self.last_ws_ping_replied_to = true
                }
                Ok(None)
            }

            Message::Text(text) => {
                let packet = serde_json::from_str(&text).map_err(Error::ReceivedMalformedPacket)?;
                self.on_packet(&packet).await?;
                Ok(Some(packet))
            }

            Message::Binary(_) => {
                self.close().await?;
                Err(Error::ReceivedBinaryMessage)
            }

            Message::Close(_) => Err(Error::ConnectionClosed),

            // We don't have to manually respond to pings.
            _ => Ok(None),
        }
    }

    async fn on_packet(&mut self, packet: &Packet) -> Result<()> {
        match packet.r#type {
            PacketType::PingEvent => self.on_ping_event(packet).await,
            PacketType::Ping => self.on_ping(packet).await,
            PacketType::PingReply => self.on_ping_reply(packet),
            _ => Ok(()),
        }
    }

    async fn on_ping_event(&mut self, packet: &Packet) -> Result<()> {
        debug!("Responding to ping-event");
        let data = packet.data.clone().unwrap_or_default();
        let data =
            serde_json::from_value::<PingEvent>(data).map_err(Error::ReceivedMalformedPacket)?;
        let time = Some(data.time);
        let reply = ParsedPacket::from_data(packet.id.clone(), PingReply { time });
        self.send(reply).await?;
        Ok(())
    }

    async fn on_ping(&mut self, packet: &Packet) -> Result<()> {
        debug!("Responding to ping");
        let data = packet.data.clone().unwrap_or_default();
        let data = serde_json::from_value::<Ping>(data).map_err(Error::ReceivedMalformedPacket)?;
        let time = Some(data.time);
        let reply = ParsedPacket::from_data(packet.id.clone(), PingReply { time });
        self.send(reply).await?;
        Ok(())
    }

    fn on_ping_reply(&mut self, packet: &Packet) -> Result<()> {
        let data = packet.data.clone().unwrap_or_default();
        let data =
            serde_json::from_value::<PingReply>(data).map_err(Error::ReceivedMalformedPacket)?;

        let Some(time) = data.time else { return Ok(()) };

        if self.last_euph_ping_payload == Some(time) {
            debug!("Received valid euph pong");
            self.last_euph_ping_replied_to = true;
        }

        Ok(())
    }
}

impl fmt::Debug for Conn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Conn")
            .field("side", &self.side)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
