use std::{collections::HashMap, sync::Arc};

use euphoxide::{
    api::ParsedPacket,
    client::{conn::ClientConnHandle, state::State},
};
use jiff::Timestamp;
use tokio::{
    select,
    sync::{mpsc, oneshot},
};

use crate::{
    Client, ClientBuilder, ClientBuilderBase, ClientConfig, ClientEvent, MultiClientConfig,
};

#[derive(Debug)]
pub enum MultiClientEvent {
    Started {
        client: Client,
    },
    Connecting {
        client: Client,
    },
    Connected {
        client: Client,
        conn: ClientConnHandle,
        state: State,
    },
    Joined {
        client: Client,
        conn: ClientConnHandle,
        state: State,
    },
    Packet {
        client: Client,
        conn: ClientConnHandle,
        state: State,
        packet: ParsedPacket,
    },
    Disconnected {
        client: Client,
    },
    Stopped {
        client: Client,
    },
}

impl MultiClientEvent {
    fn from_client_event(client: Client, event: ClientEvent) -> Self {
        match event {
            ClientEvent::Started { id: _ } => Self::Started { client },
            ClientEvent::Connecting { id: _ } => Self::Connecting { client },
            ClientEvent::Connected { id: _, conn, state } => Self::Connected {
                client,
                conn,
                state,
            },
            ClientEvent::Joined { id: _, conn, state } => Self::Joined {
                client,
                conn,
                state,
            },
            ClientEvent::Packet {
                id: _,
                conn,
                state,
                packet,
            } => Self::Packet {
                client,
                conn,
                state,
                packet,
            },
            ClientEvent::Disconnected { id: _ } => Self::Disconnected { client },
            ClientEvent::Stopped { id: _ } => Self::Stopped { client },
        }
    }

    pub fn client(&self) -> &Client {
        match self {
            Self::Started { client } => client,
            Self::Connecting { client, .. } => client,
            Self::Connected { client, .. } => client,
            Self::Joined { client, .. } => client,
            Self::Packet { client, .. } => client,
            Self::Disconnected { client } => client,
            Self::Stopped { client } => client,
        }
    }
}

#[allow(clippy::large_enum_variant)]
enum Command {
    GetClients(oneshot::Sender<Vec<Client>>),
    AddClient(ClientConfig, oneshot::Sender<Client>),
}

struct MultiClientTask {
    next_id: usize,
    clients: HashMap<usize, Client>,

    cmd_rx: mpsc::Receiver<Command>,
    event_rx: mpsc::Receiver<ClientEvent>,
    event_tx: mpsc::Sender<ClientEvent>,
    out_tx: mpsc::Sender<MultiClientEvent>,
}

impl MultiClientTask {
    fn purge_clients(&mut self) {
        self.clients.retain(|_, v| !v.stopped());
    }

    async fn on_event(&self, event: ClientEvent) {
        if let Some(client) = self.clients.get(&event.id()) {
            let event = MultiClientEvent::from_client_event(client.clone(), event);
            let _ = self.out_tx.send(event).await;
        }
    }

    async fn on_cmd(&mut self, cmd: Command) {
        match cmd {
            Command::GetClients(tx) => {
                self.purge_clients(); // Not necessary for correctness
                let _ = tx.send(self.clients.values().cloned().collect());
            }
            Command::AddClient(config, tx) => {
                let id = self.next_id;
                assert!(!self.clients.contains_key(&id));
                self.next_id += 1;

                let client = Client::new(id, config, self.event_tx.clone());
                self.clients.insert(id, client.clone());

                let _ = tx.send(client);
            }
        }
    }

    async fn run(mut self) {
        loop {
            // Prevent potential memory leak
            self.purge_clients();

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
pub struct MultiClient {
    config: Arc<MultiClientConfig>,
    cmd_tx: mpsc::Sender<Command>,
    start_time: Timestamp,
}

impl MultiClient {
    pub fn new(event_tx: mpsc::Sender<MultiClientEvent>) -> Self {
        Self::new_with_config(MultiClientConfig::default(), event_tx)
    }

    pub fn new_with_config(
        config: MultiClientConfig,
        event_tx: mpsc::Sender<MultiClientEvent>,
    ) -> Self {
        let start_time = Timestamp::now();

        let config = Arc::new(config);
        let out_tx = event_tx;

        let (cmd_tx, cmd_rx) = mpsc::channel(config.cmd_channel_bufsize);
        let (event_tx, event_rx) = mpsc::channel(config.event_channel_bufsize);

        let task = MultiClientTask {
            next_id: 0,
            clients: HashMap::new(),
            cmd_rx,
            event_rx,
            event_tx,
            out_tx,
        };

        tokio::task::spawn(task.run());

        Self {
            config,
            cmd_tx,
            start_time,
        }
    }

    pub fn config(&self) -> &MultiClientConfig {
        &self.config
    }

    pub fn start_time(&self) -> Timestamp {
        self.start_time
    }

    pub async fn get_clients(&self) -> Vec<Client> {
        let (tx, rx) = oneshot::channel();
        let _ = self.cmd_tx.send(Command::GetClients(tx)).await;
        rx.await.expect("task should still be running")
    }

    pub async fn add_client(&self, config: ClientConfig) -> Client {
        let (tx, rx) = oneshot::channel();
        let _ = self.cmd_tx.send(Command::AddClient(config, tx)).await;
        rx.await.expect("task should still be running")
    }
}

/////////////
// Builder //
/////////////

impl<'a> ClientBuilderBase<'a> for MultiClient {
    type Base = &'a Self;
}

impl MultiClient {
    pub fn client_builder(&self, room: impl ToString) -> ClientBuilder<'_, Self> {
        ClientBuilder {
            base: self,
            config: ClientConfig::new(self.config.server.clone(), room.to_string()),
        }
    }
}

impl ClientBuilder<'_, MultiClient> {
    pub async fn build_and_add(self) -> Client {
        self.base.add_client(self.config).await
    }
}
