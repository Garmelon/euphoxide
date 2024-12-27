use std::collections::HashMap;

use euphoxide::{
    api::ParsedPacket,
    client::{conn::ClientConnHandle, state::State},
};
use tokio::{
    select,
    sync::{mpsc, oneshot},
};

use crate::instance::{Event as IEvent, Instance, InstanceConfig};

#[derive(Debug)]
pub enum Event {
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

impl Event {
    fn from_instance_event(instance: Instance, event: IEvent) -> Self {
        match event {
            IEvent::Started { id: _ } => Self::Started { instance },
            IEvent::Connecting { id: _ } => Self::Connecting { instance },
            IEvent::Connected { id: _, conn, state } => Self::Connected {
                instance,
                conn,
                state,
            },
            IEvent::Joined { id: _, conn, state } => Self::Joined {
                instance,
                conn,
                state,
            },
            IEvent::Packet {
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
            IEvent::Disconnected { id: _ } => Self::Disconnected { instance },
            IEvent::Stopped { id: _ } => Self::Stopped { instance },
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

#[allow(clippy::large_enum_variant)]
enum Command {
    GetInstances(oneshot::Sender<Vec<Instance>>),
    AddInstance(InstanceConfig, oneshot::Sender<Instance>),
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct InstancesConfig {
    pub cmd_channel_bufsize: usize,
    pub event_channel_bufsize: usize,
}

impl Default for InstancesConfig {
    fn default() -> Self {
        Self {
            cmd_channel_bufsize: 1,
            event_channel_bufsize: 10,
        }
    }
}

struct InstancesTask {
    next_id: usize,
    instances: HashMap<usize, Instance>,

    cmd_rx: mpsc::Receiver<Command>,
    event_rx: mpsc::Receiver<IEvent>,
    event_tx: mpsc::Sender<IEvent>,
    out_tx: mpsc::Sender<Event>,
}

impl InstancesTask {
    fn purge_instances(&mut self) {
        self.instances.retain(|_, v| !v.stopped());
    }

    async fn on_event(&self, event: IEvent) {
        if let Some(instance) = self.instances.get(&event.id()) {
            let event = Event::from_instance_event(instance.clone(), event);
            let _ = self.out_tx.send(event).await;
        }
    }

    async fn on_cmd(&mut self, cmd: Command) {
        match cmd {
            Command::GetInstances(tx) => {
                let _ = tx.send(self.instances.values().cloned().collect());
            }
            Command::AddInstance(config, tx) => {
                let id = self.next_id;
                assert!(!self.instances.contains_key(&id));
                self.next_id += 1;

                let instance = Instance::new(id, config, self.event_tx.clone());
                self.instances.insert(id, instance.clone());

                let _ = tx.send(instance);
            }
        }
    }

    async fn run(mut self) {
        loop {
            // Prevent potential memory leak
            self.purge_instances();

            let received = select! {
                r = self.event_rx.recv() => Ok(r),
                r = self.cmd_rx.recv() => Err(r),
            };

            match received {
                Ok(None) => break,
                Ok(Some(event)) => self.on_event(event).await,
                Err(None) => break,
                Err(Some(cmd)) => self.on_cmd(cmd).await,
            }
        }
    }
}

#[derive(Clone)]
pub struct Instances {
    cmd_tx: mpsc::Sender<Command>,
}

impl Instances {
    pub fn new(config: InstancesConfig, event_tx: mpsc::Sender<Event>) -> Self {
        let out_tx = event_tx;

        let (cmd_tx, cmd_rx) = mpsc::channel(config.cmd_channel_bufsize);
        let (event_tx, event_rx) = mpsc::channel(config.event_channel_bufsize);

        let task = InstancesTask {
            next_id: 0,
            instances: HashMap::new(),
            cmd_rx,
            event_rx,
            event_tx,
            out_tx,
        };

        tokio::task::spawn(task.run());

        Self { cmd_tx }
    }

    pub async fn get_instances(&self) -> Vec<Instance> {
        let (tx, rx) = oneshot::channel();
        let _ = self.cmd_tx.send(Command::GetInstances(tx)).await;
        rx.await.expect("task should still be running")
    }

    pub async fn add_instance(&self, config: InstanceConfig) -> Instance {
        let (tx, rx) = oneshot::channel();
        let _ = self.cmd_tx.send(Command::AddInstance(config, tx)).await;
        rx.await.expect("task should still be running")
    }
}
