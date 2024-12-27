pub mod bang;
pub mod basic;
pub mod botrulez;
#[cfg(feature = "clap")]
pub mod clap;

use std::future::Future;

use async_trait::async_trait;
use euphoxide::{
    api::{self, Data, Message, MessageId, ParsedPacket, SendEvent, SendReply},
    client::{
        conn::ClientConnHandle,
        state::{Joined, State},
    },
};

use crate::{bot::BotEvent, instance::InstanceEvent};

#[non_exhaustive]
pub struct Context {
    pub conn: ClientConnHandle,
    pub joined: Joined,
}

impl Context {
    pub async fn send<S: ToString>(
        &self,
        content: S,
    ) -> euphoxide::Result<impl Future<Output = euphoxide::Result<SendReply>>> {
        self.conn
            .send(api::Send {
                content: content.to_string(),
                parent: None,
            })
            .await
    }

    pub async fn send_only<S: ToString>(&self, content: S) -> euphoxide::Result<()> {
        let _ignore = self.send(content).await?;
        Ok(())
    }

    pub async fn reply<S: ToString>(
        &self,
        parent: MessageId,
        content: S,
    ) -> euphoxide::Result<impl Future<Output = euphoxide::Result<SendReply>>> {
        self.conn
            .send(api::Send {
                content: content.to_string(),
                parent: Some(parent),
            })
            .await
    }

    pub async fn reply_only<S: ToString>(
        &self,
        parent: MessageId,
        content: S,
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
pub trait Command<B, E> {
    fn info(&self, ctx: &Context) -> Info {
        Info::default()
    }

    async fn execute(
        &self,
        arg: &str,
        msg: &Message,
        ctx: &Context,
        bot: &B,
    ) -> Result<Propagate, E>;
}

pub struct Commands<B, E = euphoxide::Error> {
    commands: Vec<Box<dyn Command<B, E> + Sync + Send>>,
}

impl<B, E> Commands<B, E> {
    pub fn new() -> Self {
        Self { commands: vec![] }
    }

    pub fn add(&mut self, command: impl Command<B, E> + Sync + Send + 'static) {
        self.commands.push(Box::new(command));
    }

    pub fn then(mut self, command: impl Command<B, E> + Sync + Send + 'static) -> Self {
        self.add(command);
        self
    }

    pub fn infos(&self, ctx: &Context) -> Vec<Info> {
        self.commands.iter().map(|c| c.info(ctx)).collect()
    }

    pub async fn on_packet(
        &self,
        conn: ClientConnHandle,
        state: State,
        packet: ParsedPacket,
        bot: &B,
    ) -> Result<Propagate, E> {
        let Ok(Data::SendEvent(SendEvent(msg))) = &packet.content else {
            return Ok(Propagate::Yes);
        };

        let State::Joined(joined) = state else {
            return Ok(Propagate::Yes);
        };

        let ctx = Context { conn, joined };

        for command in &self.commands {
            let propagate = command.execute(&msg.content, msg, &ctx, bot).await?;
            if propagate == Propagate::No {
                return Ok(Propagate::No);
            }
        }

        Ok(Propagate::Yes)
    }

    pub async fn on_instance_event(
        &self,
        event: InstanceEvent,
        bot: &B,
    ) -> Result<Propagate, E> {
        if let InstanceEvent::Packet {
            conn,
            state,
            packet,
            ..
        } = event
        {
            self.on_packet(conn, state, packet, bot).await
        } else {
            Ok(Propagate::Yes)
        }
    }

    pub async fn on_bot_event(&self, event: BotEvent, bot: &B) -> Result<Propagate, E> {
        if let BotEvent::Packet {
            conn,
            state,
            packet,
            ..
        } = event
        {
            self.on_packet(conn, state, packet, bot).await
        } else {
            Ok(Propagate::Yes)
        }
    }
}

// Has fewer restrictions on generic types than #[derive(Default)].
impl<B, E> Default for Commands<B, E> {
    fn default() -> Self {
        Self::new()
    }
}
