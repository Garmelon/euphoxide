use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use cookie::CookieJar;
use euphoxide::client::conn::ClientConnConfig;

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ServerConfig {
    pub client: ClientConnConfig,
    pub cookies: Arc<Mutex<CookieJar>>,
    pub join_attempts: usize,
    pub reconnect_delay: Duration,
    pub cmd_channel_bufsize: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            client: ClientConnConfig::default(),
            cookies: Arc::new(Mutex::new(CookieJar::new())),
            join_attempts: 5,
            reconnect_delay: Duration::from_secs(30),
            cmd_channel_bufsize: 1,
        }
    }
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ClientConfig {
    pub server: ServerConfig,
    pub room: String,
    pub human: bool,
    pub username: Option<String>,
    pub force_username: bool,
    pub password: Option<String>,
}

impl ClientConfig {
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

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct MultiClientConfig {
    pub server: ServerConfig,
    pub cmd_channel_bufsize: usize,
    pub event_channel_bufsize: usize,
}

impl Default for MultiClientConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            cmd_channel_bufsize: 1,
            event_channel_bufsize: 10,
        }
    }
}
