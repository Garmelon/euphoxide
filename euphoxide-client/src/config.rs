use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use cookie::CookieJar;
use euphoxide::client::ClientConnConfig;

/// Config options shared across [`Client`](crate::Client)s connecting to the
/// same server.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ServerConfig {
    /// Settings for the [`ClientConn`](euphoxide::client::ClientConn) used to connect to the server.
    pub client: ClientConnConfig,
    /// A [`CookieJar`] to store cookies in.
    pub cookies: Arc<Mutex<CookieJar>>,
    /// Connection attempts when first joining a room.
    ///
    /// When joining a room for the first time, attempt this many times before
    /// giving up entirely. If the client knows the room exists, then it will
    /// keep trying to reconnect regardless of this value.
    pub join_attempts: usize,
    /// Time to wait between failed connection attempts.
    pub reconnect_delay: Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            client: ClientConnConfig::default(),
            cookies: Arc::new(Mutex::new(CookieJar::new())),
            join_attempts: 5,
            reconnect_delay: Duration::from_secs(30),
        }
    }
}

/// Config options for a [`Client`](crate::Client).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ClientConfig {
    /// Server config options.
    pub server: ServerConfig,
    /// Name of the room to connect to.
    pub room: String,
    /// Whether the client should identify itself as human.
    pub human: bool,
    /// Username to set after joining.
    ///
    /// When unset, the client doesn't attempt to set a nick.
    pub username: Option<String>,
    /// Whether to update the username after joining even if it is already set.
    pub force_username: bool,
    /// Password to use for authenticating when connecting to password-protected
    /// rooms.
    pub password: Option<String>,
}

impl ClientConfig {
    /// Create a new config with default values.
    pub fn new(server: ServerConfig, room: String) -> Self {
        Self {
            server,
            room,
            human: false,
            username: None,
            force_username: false,
            password: None,
        }
    }
}

/// Config options for [`Clients`](crate::Clients).
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ClientsConfig {
    /// Server config options.
    pub server: ServerConfig,
}
