use std::{fmt::Debug, sync::Arc};

use euphoxide_client::{MultiClient, MultiClientConfig, MultiClientEvent, ServerConfig};
use jiff::Timestamp;
use log::error;
use tokio::sync::mpsc;

use crate::command::Commands;

#[non_exhaustive]
pub struct Bot<S = (), E = euphoxide::Error> {
    pub server_config: ServerConfig,
    pub commands: Arc<Commands<S, E>>,
    pub state: Arc<S>,
    pub clients: MultiClient,
    pub start_time: Timestamp,
}

impl Bot {
    pub fn new_simple(commands: Commands, event_tx: mpsc::Sender<MultiClientEvent>) -> Self {
        Self::new(
            ServerConfig::default(),
            MultiClientConfig::default(),
            commands,
            (),
            event_tx,
        )
    }
}

impl<S, E> Bot<S, E> {
    pub fn new(
        server_config: ServerConfig,
        clients_config: MultiClientConfig,
        commands: Commands<S, E>,
        state: S,
        event_tx: mpsc::Sender<MultiClientEvent>,
    ) -> Self {
        Self {
            server_config,
            commands: Arc::new(commands),
            state: Arc::new(state),
            clients: MultiClient::new_with_config(clients_config, event_tx),
            start_time: Timestamp::now(),
        }
    }
}
impl<S, E> Bot<S, E>
where
    S: Send + Sync + 'static,
    E: Debug + 'static,
{
    pub fn handle_event(&self, event: MultiClientEvent) {
        let bot = self.clone();
        tokio::task::spawn(async move {
            if let Err(err) = bot.commands.on_event(event, &bot).await {
                error!("while handling event: {err:#?}");
            }
        });
    }
}

impl<S, E> Clone for Bot<S, E> {
    fn clone(&self) -> Self {
        Self {
            server_config: self.server_config.clone(),
            commands: self.commands.clone(),
            state: self.state.clone(),
            clients: self.clients.clone(),
            start_time: self.start_time,
        }
    }
}
