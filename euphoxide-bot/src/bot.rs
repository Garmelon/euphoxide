use std::{fmt::Debug, sync::Arc};

use euphoxide_client::{MultiClient, MultiClientConfig, MultiClientEvent, ServerConfig};
use log::error;
use tokio::sync::mpsc;

use crate::command::Commands;

#[non_exhaustive]
pub struct Bot<E = euphoxide::Error> {
    pub server_config: ServerConfig,
    pub commands: Arc<Commands<E>>,
    pub clients: MultiClient,
}

impl Bot {
    pub fn new_simple(commands: Commands, event_tx: mpsc::Sender<MultiClientEvent>) -> Self {
        Self::new(
            ServerConfig::default(),
            MultiClientConfig::default(),
            commands,
            event_tx,
        )
    }
}

impl<E> Bot<E> {
    pub fn new(
        server_config: ServerConfig,
        clients_config: MultiClientConfig,
        commands: Commands<E>,
        event_tx: mpsc::Sender<MultiClientEvent>,
    ) -> Self {
        Self {
            server_config,
            commands: Arc::new(commands),
            clients: MultiClient::new_with_config(clients_config, event_tx),
        }
    }
}

impl<E> Bot<E>
where
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

impl<E> Clone for Bot<E> {
    fn clone(&self) -> Self {
        Self {
            server_config: self.server_config.clone(),
            commands: self.commands.clone(),
            clients: self.clients.clone(),
        }
    }
}
