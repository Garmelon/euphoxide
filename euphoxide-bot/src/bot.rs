use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use euphoxide::{
    api::ParsedPacket,
    client::{conn::ClientConnHandle, state::State},
};
use tokio::sync::mpsc;

use crate::{Instance, InstanceConfig, InstanceEvent};

#[derive(Debug)]
pub enum BotEvent {
    Started {
        instance: Instance,
    },
    Connecting {
        instance: Instance,
    },
    Connected {
        instance: Instance,
        conn: ClientConnHandle,
        state: State,
    },
    Joined {
        instance: Instance,
        conn: ClientConnHandle,
        state: State,
    },
    Packet {
        instance: Instance,
        conn: ClientConnHandle,
        state: State,
        packet: ParsedPacket,
    },
    Disconnected {
        instance: Instance,
    },
    Stopped {
        instance: Instance,
    },
}

impl BotEvent {
    fn from_instance_event(instance: Instance, event: InstanceEvent) -> Self {
        match event {
            InstanceEvent::Started { id: _ } => Self::Started { instance },
            InstanceEvent::Connecting { id: _ } => Self::Connecting { instance },
            InstanceEvent::Connected { id: _, conn, state } => Self::Connected {
                instance,
                conn,
                state,
            },
            InstanceEvent::Joined { id: _, conn, state } => Self::Joined {
                instance,
                conn,
                state,
            },
            InstanceEvent::Packet {
                id: _,
                conn,
                state,
                packet,
            } => Self::Packet {
                instance,
                conn,
                state,
                packet,
            },
            InstanceEvent::Disconnected { id: _ } => Self::Disconnected { instance },
            InstanceEvent::Stopped { id: _ } => Self::Stopped { instance },
        }
    }

    pub fn instance(&self) -> &Instance {
        match self {
            Self::Started { instance } => instance,
            Self::Connecting { instance, .. } => instance,
            Self::Connected { instance, .. } => instance,
            Self::Joined { instance, .. } => instance,
            Self::Packet { instance, .. } => instance,
            Self::Disconnected { instance } => instance,
            Self::Stopped { instance } => instance,
        }
    }
}

#[non_exhaustive]
pub struct BotConfig {
    pub event_timeout: Duration,
    pub event_channel_bufsize: usize,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            event_timeout: Duration::from_secs(1),
            event_channel_bufsize: 10,
        }
    }
}

pub struct Bot {
    config: BotConfig,
    next_id: usize,
    instances: Arc<RwLock<HashMap<usize, Instance>>>,
    event_tx: mpsc::Sender<InstanceEvent>,
    event_rx: mpsc::Receiver<InstanceEvent>,
}

impl Bot {
    pub fn new() -> Self {
        Self::new_with_config(BotConfig::default())
    }

    pub fn new_with_config(config: BotConfig) -> Self {
        let (event_tx, event_rx) = mpsc::channel(10);
        Self {
            config,
            next_id: 0,
            instances: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            event_rx,
        }
    }

    fn purge_instances(&self) {
        let mut guard = self.instances.write().unwrap();
        guard.retain(|_, v| !v.stopped());
    }

    pub fn get_instances(&self) -> Vec<Instance> {
        self.instances.read().unwrap().values().cloned().collect()
    }

    pub fn add_instance(&mut self, config: InstanceConfig) -> Instance {
        let id = self.next_id;
        self.next_id += 1;

        let mut guard = self.instances.write().unwrap();
        assert!(!guard.contains_key(&id));

        let instance = Instance::new(id, config, self.event_tx.clone());
        guard.insert(id, instance.clone());

        instance
    }

    pub async fn recv(&mut self) -> Option<BotEvent> {
        // We hold exactly one sender. If no other senders exist, then all
        // instances are dead and we'll never receive any more events unless we
        // return and allow the user to add more instances again.
        while self.event_rx.sender_strong_count() > 1 {
            // Prevent potential memory leak
            self.purge_instances();

            let Ok(event) =
                tokio::time::timeout(self.config.event_timeout, self.event_rx.recv()).await
            else {
                // We need to re-check the sender count occasionally. It's
                // possible that there are still instances that just haven't
                // sent an event in a while, so we can't just return here.
                continue;
            };

            // This only returns None if no senders remain, and since we always
            // own one sender, this can't happen.
            let event = event.expect("event channel should never close since we own a sender");

            if let Some(instance) = self.instances.read().unwrap().get(&event.id()) {
                return Some(BotEvent::from_instance_event(instance.clone(), event));
            }
        }

        None
    }
}

impl Default for Bot {
    fn default() -> Self {
        Self::new()
    }
}
