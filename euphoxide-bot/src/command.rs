pub mod bang;
pub mod basic;
pub mod botrulez;
#[cfg(feature = "clap")]
pub mod clap;

use std::{future::Future, sync::Arc};

use async_trait::async_trait;
use euphoxide::{
    api::{self, Data, Message, MessageId, SendEvent, SendReply},
    client::{
        conn::ClientConnHandle,
        state::{Joined, State},
    },
};
use euphoxide_client::{Client, MultiClient, MultiClientEvent};

#[non_exhaustive]
pub struct Context<E = euphoxide::Error> {
    pub commands: Arc<Commands<E>>,
    pub clients: MultiClient,
    pub client: Client,
    pub conn: ClientConnHandle,
    pub joined: Joined,
}

impl<E> Context<E> {
    pub async fn send(
        &self,
        content: impl ToString,
    ) -> euphoxide::Result<impl Future<Output = euphoxide::Result<SendReply>>> {
        self.conn
            .send(api::Send {
                content: content.to_string(),
                parent: None,
            })
            .await
    }

    pub async fn send_only(&self, content: impl ToString) -> euphoxide::Result<()> {
        let _ignore = self.send(content).await?;
        Ok(())
    }

    pub async fn reply(
        &self,
        parent: MessageId,
        content: impl ToString,
    ) -> euphoxide::Result<impl Future<Output = euphoxide::Result<SendReply>>> {
        self.conn
            .send(api::Send {
                content: content.to_string(),
                parent: Some(parent),
            })
            .await
    }

    pub async fn reply_only(
        &self,
        parent: MessageId,
        content: impl ToString,
    ) -> euphoxide::Result<()> {
        let _ignore = self.reply(parent, content).await?;
        Ok(())
    }
}

#[derive(Default)]
pub struct Info {
    pub trigger: Option<String>,
    pub description: Option<String>,
}

impl Info {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_trigger(mut self, trigger: impl ToString) -> Self {
        self.trigger = Some(trigger.to_string());
        self
    }

    pub fn with_description(mut self, description: impl ToString) -> Self {
        self.description = Some(description.to_string());
        self
    }

    pub fn prepend_trigger(&mut self, trigger: impl ToString) {
        let cur_trigger = self.trigger.get_or_insert_default();
        if !cur_trigger.is_empty() {
            cur_trigger.insert(0, ' ');
        }
        cur_trigger.insert_str(0, &trigger.to_string());
    }

    pub fn with_prepended_trigger(mut self, trigger: impl ToString) -> Self {
        self.prepend_trigger(trigger);
        self
    }
}

/// Whether a message should propagate to subsequent commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Propagate {
    No,
    Yes,
}

#[allow(unused_variables)]
#[async_trait]
pub trait Command<E = euphoxide::Error> {
    fn info(&self, ctx: &Context<E>) -> Info {
        Info::default()
    }

    async fn execute(&self, arg: &str, msg: &Message, ctx: &Context<E>) -> Result<Propagate, E>;
}

pub struct Commands<E = euphoxide::Error> {
    commands: Vec<Box<dyn Command<E> + Sync + Send>>,
}

impl<E> Commands<E> {
    pub fn new() -> Self {
        Self { commands: vec![] }
    }

    pub fn add(&mut self, command: impl Command<E> + Sync + Send + 'static) {
        self.commands.push(Box::new(command));
    }

    pub fn then(mut self, command: impl Command<E> + Sync + Send + 'static) -> Self {
        self.add(command);
        self
    }

    pub fn build(self) -> Arc<Self> {
        Arc::new(self)
    }

    pub fn infos(&self, ctx: &Context<E>) -> Vec<Info> {
        self.commands.iter().map(|c| c.info(ctx)).collect()
    }

    pub async fn handle_message(
        self: Arc<Self>,
        clients: MultiClient,
        client: Client,
        conn: ClientConnHandle,
        joined: Joined,
        msg: &Message,
    ) -> Result<Propagate, E> {
        let ctx = Context {
            commands: self.clone(),
            clients,
            client,
            conn,
            joined,
        };

        for command in &self.commands {
            let propagate = command.execute(&msg.content, msg, &ctx).await?;
            if propagate == Propagate::No {
                return Ok(Propagate::No);
            }
        }

        Ok(Propagate::Yes)
    }

    pub async fn handle_event(
        self: Arc<Self>,
        clients: MultiClient,
        event: MultiClientEvent,
    ) -> Result<Propagate, E> {
        let MultiClientEvent::Packet {
            client,
            conn,
            state,
            packet,
        } = event
        else {
            return Ok(Propagate::Yes);
        };

        let Ok(Data::SendEvent(SendEvent(msg))) = &packet.content else {
            return Ok(Propagate::Yes);
        };

        let State::Joined(joined) = state else {
            return Ok(Propagate::Yes);
        };

        self.handle_message(clients, client, conn, joined, msg)
            .await
    }
}

// Has fewer restrictions on generic types than #[derive(Default)].
impl<E> Default for Commands<E> {
    fn default() -> Self {
        Self::new()
    }
}
