use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use cookie::CookieJar;
use euphoxide::client::conn::ClientConnConfig;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub client: ClientConnConfig,
    pub cookies: Arc<Mutex<CookieJar>>,
    pub join_attempts: usize,
    pub reconnect_delay: Duration,
    pub cmd_channel_bufsize: usize,
    pub event_channel_bufsize: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            client: ClientConnConfig::default(),
            cookies: Arc::new(Mutex::new(CookieJar::new())),
            join_attempts: 5,
            reconnect_delay: Duration::from_secs(30),
            cmd_channel_bufsize: 1,
            event_channel_bufsize: 10,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstanceConfig {
    pub server: ServerConfig,
    pub room: String,
    pub human: bool,
    pub username: Option<String>,
    pub force_username: bool,
    pub password: Option<String>,
}

impl InstanceConfig {
    pub fn new(room: impl ToString) -> Self {
        Self {
            server: ServerConfig::default(),
            room: room.to_string(),
            human: false,
            username: None,
            force_username: false,
            password: None,
        }
    }

    pub fn with_username(mut self, username: impl ToString) -> Self {
        self.username = Some(username.to_string());
        self
    }

    pub fn with_force_username(mut self, enabled: bool) -> Self {
        self.force_username = enabled;
        self
    }

    pub fn with_password(mut self, password: impl ToString) -> Self {
        self.password = Some(password.to_string());
        self
    }
}