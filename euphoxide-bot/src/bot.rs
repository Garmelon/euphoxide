use std::{fmt::Debug, sync::Arc};

use jiff::Timestamp;
use log::error;
use tokio::sync::mpsc;

use crate::{
    command::Commands,
    instance::ServerConfig,
    instances::{Event, Instances, InstancesConfig},
};

#[non_exhaustive]
pub struct Bot<S = (), E = euphoxide::Error> {
    pub server_config: ServerConfig,
    pub commands: Arc<Commands<S, E>>,
    pub state: Arc<S>,
    pub instances: Instances,
    pub start_time: Timestamp,
}

impl Bot {
    pub fn new_simple(commands: Commands, event_tx: mpsc::Sender<Event>) -> Self {
        Self::new(
            ServerConfig::default(),
            InstancesConfig::default(),
            commands,
            (),
            event_tx,
        )
    }
}

impl<S, E> Bot<S, E> {
    pub fn new(
        server_config: ServerConfig,
        instances_config: InstancesConfig,
        commands: Commands<S, E>,
        state: S,
        event_tx: mpsc::Sender<Event>,
    ) -> Self {
        Self {
            server_config,
            commands: Arc::new(commands),
            state: Arc::new(state),
            instances: Instances::new(instances_config, event_tx),
            start_time: Timestamp::now(),
        }
    }
}
impl<S, E> Bot<S, E>
where
    S: Send + Sync + 'static,
    E: Debug + 'static,
{
    pub fn handle_event(&self, event: Event) {
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
            instances: self.instances.clone(),
            start_time: self.start_time,
        }
    }
}
