//! Error handling.

use std::fmt;

use tokio_tungstenite::tungstenite;

use crate::api::PacketType;

/// Possible euphoria communication errors.
#[derive(Debug)]
pub enum Error {
    /// The connection is closed.
    ConnectionClosed,

    /// A ping was not replied to in time.
    PingTimeout,

    /// A packet was not sent because it was malformed.
    MalformedPacket(serde_json::Error),

    /// A binary message was received.
    ReceivedBinaryMessage,

    /// A malformed packet was received.
    ReceivedMalformedPacket(serde_json::Error),

    /// An unexpected packet was received.
    ReceivedUnexpectedPacket(PacketType),

    /// A tungstenite error.
    Tungstenite(tungstenite::Error),

    /// A timeout occurred while opening a connection.
    ///
    /// This is a higher-level error that only occurs in the
    /// [`ClientConn`](crate::client::conn::ClientConn).
    ConnectionTimeout,

    /// The server did not reply to a command in time.
    ///
    /// This is a higher-level error that only occurs with
    /// [`Command`](crate::api::Command)-based APIs in
    /// [`ClientConn`](crate::client::conn::ClientConn).
    CommandTimeout,

    /// The server replied with an error string.
    ///
    /// This is a higher-level error that only occurs with
    /// [`Command`](crate::api::Command)-based APIs in
    /// [`ClientConn`](crate::client::conn::ClientConn).
    Euph(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionClosed => write!(f, "connection closed"),
            Self::PingTimeout => write!(f, "ping timed out"),
            Self::MalformedPacket(err) => write!(f, "malformed packet: {err}"),
            Self::ReceivedBinaryMessage => write!(f, "received binary message"),
            Self::ReceivedMalformedPacket(err) => write!(f, "received malformed packet: {err}"),
            Self::ReceivedUnexpectedPacket(ptype) => {
                write!(f, "received packet of unexpected type: {ptype}")
            }
            Self::Tungstenite(err) => write!(f, "{err}"),
            Self::ConnectionTimeout => write!(f, "connection timed out while connecting"),
            Self::CommandTimeout => write!(f, "command timed out"),
            Self::Euph(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<tungstenite::Error> for Error {
    fn from(err: tungstenite::Error) -> Self {
        Self::Tungstenite(err)
    }
}

/// An alias of [`Result`](std::result::Result) for [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
