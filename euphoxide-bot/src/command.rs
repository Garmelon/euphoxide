pub mod bang;
pub mod basic;
pub mod botrulez;
#[cfg(feature = "clap")]
pub mod clap;

use std::future::Future;

use async_trait::async_trait;
use euphoxide::{
    api::{self, Data, Message, MessageId, SendEvent, SendReply},
    client::{
        conn::ClientConnHandle,
        state::{Joined, State},
    },
};

use crate::{bot::Bot, instance::Instance, instances::Event};

#[non_exhaustive]
pub struct Context {
    pub instance: Instance,
    pub conn: ClientConnHandle,
    pub joined: Joined,
}

impl Context {
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
pub trait Command<S = (), E = euphoxide::Error> {
    fn info(&self, ctx: &Context) -> Info {
        Info::default()
    }

    async fn execute(
        &self,
        arg: &str,
        msg: &Message,
        ctx: &Context,
        bot: &Bot<S, E>,
    ) -> Result<Propagate, E>;
}

pub trait CommandExt: Sized {
    fn described(self) -> basic::Described<Self> {
        basic::Described::new(self)
    }

    fn hidden(self) -> basic::Described<Self> {
        basic::Described::hidden(self)
    }

    fn prefixed(self, prefix: impl ToString) -> basic::Prefixed<Self> {
        basic::Prefixed::new(prefix, self)
    }

    fn general(self, name: impl ToString) -> bang::General<Self> {
        bang::General::new(name, self)
    }

    fn global(self, name: impl ToString) -> bang::Global<Self> {
        bang::Global::new(name, self)
    }

    fn specific(self, name: impl ToString) -> bang::Specific<Self> {
        bang::Specific::new(name, self)
    }
}

impl<C> CommandExt for C {}

pub struct Commands<S = (), E = euphoxide::Error> {
    commands: Vec<Box<dyn Command<S, E> + Sync + Send>>,
}

impl<S, E> Commands<S, E> {
    pub fn new() -> Self {
        Self { commands: vec![] }
    }

    pub fn add(&mut self, command: impl Command<S, E> + Sync + Send + 'static) {
        self.commands.push(Box::new(command));
    }

    pub fn then(mut self, command: impl Command<S, E> + Sync + Send + 'static) -> Self {
        self.add(command);
        self
    }

    pub fn infos(&self, ctx: &Context) -> Vec<Info> {
        self.commands.iter().map(|c| c.info(ctx)).collect()
    }

    pub(crate) async fn on_event(&self, event: Event, bot: &Bot<S, E>) -> Result<Propagate, E> {
        let Event::Packet {
            instance,
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

        let ctx = Context {
            instance,
            conn,
            joined,
        };

        for command in &self.commands {
            let propagate = command.execute(&msg.content, msg, &ctx, bot).await?;
            if propagate == Propagate::No {
                return Ok(Propagate::No);
            }
        }

        Ok(Propagate::Yes)
    }
}

// Has fewer restrictions on generic types than #[derive(Default)].
impl<S, E> Default for Commands<S, E> {
    fn default() -> Self {
        Self::new()
    }
}
