//! Models the client's connection state.

use std::collections::HashMap;

use jiff::Timestamp;
use log::debug;

use crate::api::{
    BounceEvent, Data, HelloEvent, NickEvent, PersonalAccountView, SessionId, SessionView,
    SnapshotEvent, UserId,
};

/// Information about a session in the room.
///
/// For quite a while before finally going down altogether, the euphoria.io
/// instance had an unreliable nick list: Listings returned by the server were
/// usually incomplete. Because of this, the bot library uses any observable
/// action by a session (including nick changes) to update the listing. Since
/// nick events don't include full session info though, the [`SessionInfo`] enum
/// can contain partial information.
///
/// This level of paranioa probably isn't required any more now that the only
/// euphoria instance is working correctly. However, the code already works and
/// users who don't want to worry about it can just ignore partial session
/// infos.
#[derive(Debug, Clone)]
pub enum SessionInfo {
    Full(SessionView),
    Partial(NickEvent),
}

impl SessionInfo {
    /// Retrieve the user id of the session.
    pub fn id(&self) -> &UserId {
        match self {
            Self::Full(sess) => &sess.id,
            Self::Partial(nick) => &nick.id,
        }
    }

    /// Retrieve the session id of the session.
    pub fn session_id(&self) -> &SessionId {
        match self {
            Self::Full(sess) => &sess.session_id,
            Self::Partial(nick) => &nick.session_id,
        }
    }

    /// Retrieve the user name of the session.
    pub fn name(&self) -> &str {
        match self {
            Self::Full(sess) => &sess.name,
            Self::Partial(nick) => &nick.to,
        }
    }
}

impl From<SessionView> for SessionInfo {
    fn from(value: SessionView) -> Self {
        Self::Full(value)
    }
}

impl From<NickEvent> for SessionInfo {
    fn from(value: NickEvent) -> Self {
        Self::Partial(value)
    }
}

/// The state of the connection before the client has joined the room.
///
/// Depending on the room, the client may need to authenticate or log in in
/// order to join.
#[derive(Debug, Clone)]
pub struct Joining {
    /// Since when the connection has been in this state.
    pub since: Timestamp,
    /// A [`HelloEvent`], if one has been received.
    ///
    /// Contains information about the client's own session.
    pub hello: Option<HelloEvent>,
    /// A [`SnapshotEvent`], if one has been received.
    pub snapshot: Option<SnapshotEvent>,
    /// A [`BounceEvent`], if one has been received.
    pub bounce: Option<BounceEvent>,
}

impl Joining {
    fn new() -> Self {
        Self {
            since: Timestamp::now(),
            hello: None,
            snapshot: None,
            bounce: None,
        }
    }

    fn on_data(&mut self, data: &Data) {
        match data {
            Data::BounceEvent(p) => self.bounce = Some(p.clone()),
            Data::HelloEvent(p) => self.hello = Some(p.clone()),
            Data::SnapshotEvent(p) => self.snapshot = Some(p.clone()),
            _ => {}
        }
    }

    fn to_joined(&self) -> Option<Joined> {
        let hello = self.hello.as_ref()?;
        let snapshot = self.snapshot.as_ref()?;

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
            since: Timestamp::now(),
            session,
            account: hello.account.clone(),
            listing,
        })
    }
}

/// The state of the connection after the client has successfully joined the
/// room.
///
/// The client may need to set a nick in order to be able to send messages.
/// However, it can access the room history without nick.
#[derive(Debug, Clone)]
pub struct Joined {
    /// Since when the connection has been in this state.
    pub since: Timestamp,
    /// The client's own session.
    pub session: SessionView,
    /// Account information, if the client is logged in.
    pub account: Option<PersonalAccountView>,
    /// All sessions currently connected to the room (except the client's own
    /// session).
    pub listing: HashMap<SessionId, SessionInfo>,
}

impl Joined {
    fn on_data(&mut self, data: &Data) {
        match data {
            Data::JoinEvent(p) => {
                debug!("Updating listing after join-event");
                self.listing
                    .insert(p.0.session_id.clone(), SessionInfo::Full(p.0.clone()));
            }
            Data::PartEvent(p) => {
                debug!("Updating listing after part-event");
                self.listing.remove(&p.0.session_id);
            }
            Data::NetworkEvent(p) => {
                if p.r#type == "partition" {
                    debug!("Updating listing after network-event with type partition");
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
            Data::SendEvent(p) => {
                debug!("Updating listing after send-event");
                self.listing.insert(
                    p.0.sender.session_id.clone(),
                    SessionInfo::Full(p.0.sender.clone()),
                );
            }
            Data::NickEvent(p) => {
                debug!("Updating listing after nick-event");
                self.listing
                    .entry(p.session_id.clone())
                    .and_modify(|s| match s {
                        SessionInfo::Full(session) => session.name = p.to.clone(),
                        SessionInfo::Partial(_) => *s = SessionInfo::Partial(p.clone()),
                    })
                    .or_insert_with(|| SessionInfo::Partial(p.clone()));
            }
            Data::NickReply(p) => {
                debug!("Updating own session after nick-reply");
                assert_eq!(self.session.id, p.id);
                self.session.name = p.to.clone();
            }
            Data::WhoReply(p) => {
                debug!("Updating listing after who-reply");
                self.listing.clear();
                for session in p.listing.clone() {
                    if session.session_id == self.session.session_id {
                        self.session = session;
                    } else {
                        self.listing
                            .insert(session.session_id.clone(), session.into());
                    }
                }
            }
            _ => {}
        }
    }
}

/// The state of a connection to the server, from a client's perspective.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum State {
    /// The client has not joined the room yet.
    Joining(Joining),
    /// The client has successfully joined the room.
    Joined(Joined),
}

impl State {
    /// Create a new state for a fresh connection.
    ///
    /// Assumes that no packets have been received yet. See also
    /// [`Self::on_data`].
    pub fn new() -> Self {
        Joining::new().into()
    }

    /// If the state consists of a [`Joining`], return a reference to it.
    pub fn as_joining(&self) -> Option<&Joining> {
        match self {
            Self::Joining(joining) => Some(joining),
            Self::Joined(_) => None,
        }
    }

    /// If the state consists of a [`Joined`], return a reference to it.
    pub fn as_joined(&self) -> Option<&Joined> {
        match self {
            Self::Joining(_) => None,
            Self::Joined(joined) => Some(joined),
        }
    }

    /// If the state consists of a [`Joining`], return it.
    pub fn into_joining(self) -> Option<Joining> {
        match self {
            Self::Joining(joining) => Some(joining),
            Self::Joined(_) => None,
        }
    }

    /// If the state consists of a [`Joined`], return it.
    pub fn into_joined(self) -> Option<Joined> {
        match self {
            Self::Joining(_) => None,
            Self::Joined(joined) => Some(joined),
        }
    }

    /// Update the state with a packet received from the server.
    ///
    /// This method should be called whenever any packet is received from the
    /// server. Skipping packets may cause the state to become inconsistent.
    pub fn on_data(&mut self, data: &Data) {
        match self {
            Self::Joining(joining) => {
                joining.on_data(data);
                if let Some(joined) = joining.to_joined() {
                    *self = joined.into();
                }
            }
            Self::Joined(joined) => joined.on_data(data),
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Joining> for State {
    fn from(value: Joining) -> Self {
        Self::Joining(value)
    }
}

impl From<Joined> for State {
    fn from(value: Joined) -> Self {
        Self::Joined(value)
    }
}
