//! Chat room commands.
//!
//! These commands are available to the client once a session successfully joins
//! a room.

use serde::{Deserialize, Serialize};

use super::{Message, MessageId, PmId, SessionId, SessionView, UserId};

/// Retrieve the full content of a single message in the room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMessage {
    /// The id of the message to retrieve.
    pub id: MessageId,
}

/// The message retrieved by [`GetMessage`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMessageReply(pub Message);

/// Request messages from the room's message log.
///
/// This can be used to supplement the log provided by
/// [`SnapshotEvent`](super::SnapshotEvent) (for example, when scrolling back
/// further in history).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Log {
    /// Maximum number of messages to return (up to 1000).
    pub n: usize,
    /// Return messages prior to this snowflake.
    pub before: Option<MessageId>,
}

/// List of messages from the room's message log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogReply {
    /// List of messages returned.
    pub log: Vec<Message>,
    /// Messages prior to this snowflake were returned.
    pub before: Option<MessageId>,
}

/// Set the name you present to the room.
///
/// This name applies to all messages sent during this session, until the nick
/// command is called again.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nick {
    /// The requested name (maximum length 36 bytes).
    pub name: String,
}

/// Confirms the [`Nick`] command.
///
/// Returns the session's former and new names (the server may modify the
/// requested nick).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NickReply {
    /// The id of the session this name applies to.
    pub session_id: SessionId,
    /// The id of the agent or account logged into the session.
    pub id: UserId,
    /// The previous name associated with the session.
    pub from: String,
    /// The name associated with the session henceforth.
    pub to: String,
}

/// Constructs a virtual room for private messaging between the client and the
/// given [`UserId`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PmInitiate {
    /// The id of the user to invite to chat privately.
    pub user_id: UserId,
}

/// Provides the PMID for the requested private messaging room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PmInitiateReply {
    /// The private chat can be accessed at `/room/pm:<pm_id>`.
    pub pm_id: PmId,
    /// The nickname of the recipient of the invitation.
    pub to_nick: String,
}

/// Send a message to a room.
///
/// The session must be successfully joined with the room. This message will be
/// broadcast to all sessions joined with the room.
///
/// If the room is private, then the message content will be encrypted before it
/// is stored and broadcast to the rest of the room.
///
/// The caller of this command will not receive the corresponding
/// [`SendEvent`](super::SendEvent), but will receive the same information in
/// the [`SendReply`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Send {
    /// The content of the message (client-defined).
    pub content: String,
    /// The id of the parent message, if any.
    pub parent: Option<MessageId>,
}

/// The message that was sent.
///
/// this includes the message id, which was populated by the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendReply(pub Message);

/// Request a list of sessions currently joined in the room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Who {}

/// Lists the sessions currently joined in the room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhoReply {
    /// A list of session views.
    pub listing: Vec<SessionView>,
}
