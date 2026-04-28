use std::{collections::HashMap, sync::Arc};

use jiff::Timestamp;
use tokio::{
    select,
    sync::{mpsc, oneshot},
};

use crate::{Client, ClientBuilder, ClientConfig, ClientEvent, ClientsConfig};

enum Command {
    GetClients(oneshot::Sender<Vec<Client>>),
    AddClient(ClientConfig, oneshot::Sender<Client>),
}

struct ClientsTask {
    next_id: usize,
    clients: HashMap<usize, Client>,

    cmd_rx: mpsc::UnboundedReceiver<Command>,
    event_rx: mpsc::Receiver<(usize, ClientEvent)>,
    event_tx: mpsc::Sender<(usize, ClientEvent)>,
    out_tx: mpsc::Sender<(Client, ClientEvent)>,
}

impl ClientsTask {
    fn purge_clients(&mut self) {
        self.clients.retain(|_, v| !v.stopped());
    }

    async fn on_event(&self, client_id: usize, event: ClientEvent) {
        if let Some(client) = self.clients.get(&client_id) {
            let _ = self.out_tx.send((client.clone(), event)).await;
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
                Ok(Some((client_id, event))) => self.on_event(client_id, event).await,
                Err(None) => break,
                Err(Some(cmd)) => self.on_cmd(cmd).await,
            }
        }
    }
}

/// A collection of multiple [`Client`]s.
#[derive(Clone)]
pub struct Clients {
    config: Arc<ClientsConfig>,
    cmd_tx: mpsc::UnboundedSender<Command>,
    start_time: Timestamp,
}

impl Clients {
    /// Create a new client collection with default config.
    ///
    /// The clients will report events to the provided mpsc queue.
    pub fn new(event_tx: mpsc::Sender<(Client, ClientEvent)>) -> Self {
        Self::new_with_config(ClientsConfig::default(), event_tx)
    }

    /// Create a new client collection.
    ///
    /// The clients will report events to the provided mpsc queue.
    pub fn new_with_config(
        config: ClientsConfig,
        event_tx: mpsc::Sender<(Client, ClientEvent)>,
    ) -> Self {
        let start_time = Timestamp::now();

        let config = Arc::new(config);
        let out_tx = event_tx;

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::channel(config.event_channel_bufsize);

        let task = ClientsTask {
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

    /// The client collection's config.
    pub fn config(&self) -> &ClientsConfig {
        &self.config
    }

    /// The time the client collection was created.
    pub fn start_time(&self) -> Timestamp {
        self.start_time
    }

    /// Get all currently active clients.
    pub async fn get_clients(&self) -> Vec<Client> {
        let (tx, rx) = oneshot::channel();
        let _ = self.cmd_tx.send(Command::GetClients(tx));
        rx.await.expect("task should still be running")
    }

    /// Add a new client to the collection.
    ///
    /// The client must have a unique ID.
    pub async fn add_client(&self, config: ClientConfig) -> Client {
        let (tx, rx) = oneshot::channel();
        let _ = self.cmd_tx.send(Command::AddClient(config, tx));
        rx.await.expect("task should still be running")
    }
}

/////////////
// Builder //
/////////////

impl Clients {
    /// Create a builder for a new client.
    pub fn client_builder(&self, room: impl ToString) -> ClientBuilder<&Self> {
        ClientBuilder {
            base: self,
            config: ClientConfig::new(self.config.server.clone(), room.to_string()),
        }
    }
}

impl ClientBuilder<&Clients> {
    /// Build a client and add it to the client collection the builder was
    /// created from.
    pub async fn build_and_add(self) -> Client {
        self.base.add_client(self.config).await
    }
}
