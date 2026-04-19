use crate::ClientConfig;

pub struct ClientBuilder<B> {
    pub(crate) base: B,
    pub(crate) config: ClientConfig,
}

impl<B> ClientBuilder<B> {
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut ClientConfig {
        &mut self.config
    }

    pub fn with_human(mut self, human: bool) -> Self {
        self.config.human = human;
        self
    }

    pub fn with_username(mut self, username: impl ToString) -> Self {
        self.config.username = Some(username.to_string());
        self
    }

    pub fn with_force_username(mut self, force_username: bool) -> Self {
        self.config.force_username = force_username;
        self
    }

    pub fn with_password(mut self, password: impl ToString) -> Self {
        self.config.password = Some(password.to_string());
        self
    }
}
