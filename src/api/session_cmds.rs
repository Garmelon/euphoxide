//! Models [session commands][0] and their replies.
//!
//! Session management commands are involved in the initial handshake and
//! maintenance of a session.
//!
//! [0]: https://euphoria.leet.nu/heim/api#session-commands

use serde::{Deserialize, Serialize};

use super::{AuthOption, Time};

/// Attempt to join a private room.
///
/// This should be sent in response to a [`BounceEvent`](super::BounceEvent) at
/// the beginning of a session.
///
/// <https://euphoria.leet.nu/heim/api#auth>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Auth {
    /// The method of authentication.
    pub r#type: AuthOption,
    /// Use this field for [`AuthOption::Passcode`] authentication.
    pub passcode: Option<String>,
}

/// Reports whether the [`Auth`] command succeeded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthReply {
    /// True if authentication succeeded.
    pub success: bool,
    /// If [`Self::success`] was false, the reason for failure.
    pub reason: Option<String>,
}

/// Initiate a client-to-server ping.
///
/// The server will send back a [`PingReply`] with the same timestamp as soon as
/// possible.
///
/// <https://euphoria.leet.nu/heim/api#ping>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ping {
    /// An arbitrary value, intended to be a unix timestamp.
    pub time: Time,
}

/// Response to a [`Ping`] command or [`PingEvent`](super::PingEvent).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingReply {
    /// The timestamp of the ping being replied to.
    pub time: Option<Time>,
}
