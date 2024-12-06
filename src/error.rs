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
