use std::sync::Arc;

use jiff::Timestamp;
use tokio::sync::mpsc;

use crate::{
    instance::ServerConfig,
    instances::{Event, Instances, InstancesConfig},
};

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct BotConfig<S> {
    pub server: ServerConfig,
    pub instances: InstancesConfig,
    pub state: S,
}

impl<S> BotConfig<S> {
    pub fn with_state<S2>(self, state: S2) -> BotConfig<S2> {
        BotConfig {
            server: self.server,
            instances: self.instances,
            state,
        }
    }

    pub fn create(self, event_tx: mpsc::Sender<Event>) -> Bot<S> {
        Bot::new(self, event_tx)
    }
}

impl Default for BotConfig<()> {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            instances: InstancesConfig::default(),
            state: (),
        }
    }
}

#[derive(Clone)]
pub struct Bot<S> {
    pub server_config: ServerConfig,
    pub state: Arc<S>,
    pub instances: Instances,
    pub start_time: Timestamp,
}

impl<S> Bot<S> {
    pub fn new(config: BotConfig<S>, event_tx: mpsc::Sender<Event>) -> Self {
        Self {
            server_config: config.server,
            state: Arc::new(config.state),
            instances: Instances::new(config.instances, event_tx),
            start_time: Timestamp::now(),
        }
    }

    pub fn handle_event(&self, event: Event) {
        todo!()
    }
}
