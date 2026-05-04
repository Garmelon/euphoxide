use crate::ClientConfig;

/// Builder for a [`Client`](crate::Client) or a
/// [`Clients`](crate::Clients).
///
/// Create a builder using [`Client::builder`](crate::Client::builder) or
/// [`Clients::client_builder`](crate::Clients::client_builder).
#[derive(Clone)]
pub struct ClientBuilder<B> {
    pub(crate) base: B,
    pub(crate) config: ClientConfig,
}

impl<B> ClientBuilder<B> {
    /// The current [`ClientConfig`].
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// A mutable reference to the current [`ClientConfig`].
    pub fn config_mut(&mut self) -> &mut ClientConfig {
        &mut self.config
    }

    /// Set [`ClientConnConfig::human`](euphoxide::client::ClientConnConfig::human).
    pub fn with_human(mut self, human: bool) -> Self {
        self.config.server.client.human = human;
        self
    }

    /// Set [`ClientConfig::username`].
    pub fn with_username(mut self, username: impl ToString) -> Self {
        self.config.username = Some(username.to_string());
        self
    }

    /// Set [`ClientConfig::force_username`].
    pub fn with_force_username(mut self, force_username: bool) -> Self {
        self.config.force_username = force_username;
        self
    }

    /// Set [`ClientConfig::password`].
    pub fn with_password(mut self, password: impl ToString) -> Self {
        self.config.password = Some(password.to_string());
        self
    }
}
