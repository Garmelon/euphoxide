//! A bot command framework for `euphoxide`.
//!
//! It is considered experimental for now.

pub mod bang;
pub mod basic;
pub mod botrulez;
#[cfg(feature = "clap")]
pub mod clap;

use std::sync::Arc;

use async_trait::async_trait;
use euphoxide::{
    api::{self, Data, Message, MessageId, SendEvent, SendReply},
    client::{ClientConnHandle, Joined, State},
};
use euphoxide_client::{Client, ClientEvent, Clients};

use self::{
    bang::{General, Global, Specific},
    basic::{Described, Prefixed},
};

/// Execution context for commands.
///
/// See [`Command`] for more details.
#[non_exhaustive]
pub struct Context<D = (), E = euphoxide::Error> {
    /// The [`Commands`] instance the command is a part of.
    pub commands: Arc<Commands<D, E>>,
    /// The [`Clients`] instance making up the bot.
    pub clients: Clients,
    /// The [`Client`] that received the command.
    pub client: Client,
    /// The connection the command was received on.
    pub conn: ClientConnHandle,
    /// The room state at the time the command was received.
    pub joined: Joined,
    /// The message containing the command.
    pub msg: Message,
}

impl<D, E> Context<D, E> {
    /// Retrieve the user-supplied application data from [`Self::commands`].
    pub fn data(&self) -> &D {
        &self.commands.data
    }

    /// Send a message to the room the command was received in.
    pub async fn send(
        &self,
        parent: Option<MessageId>,
        content: impl ToString,
    ) -> euphoxide::Result<impl Future<Output = euphoxide::Result<SendReply>>> {
        self.conn
            .send(api::Send {
                content: content.to_string(),
                parent,
            })
            .await
    }

    /// Like [`Self::send`], but ignoring the server's reply.
    ///
    /// This saves you from having to write `let _ =` to silence warnings.
    pub async fn send_only(
        &self,
        parent: Option<MessageId>,
        content: impl ToString,
    ) -> euphoxide::Result<()> {
        let _ignore = self.send(parent, content).await?;
        Ok(())
    }

    /// Send a reply to the message that triggered the command.
    pub async fn reply(
        &self,
        content: impl ToString,
    ) -> euphoxide::Result<impl Future<Output = euphoxide::Result<SendReply>>> {
        self.send(Some(self.msg.id), content).await
    }

    /// Like [`Self::reply`], but ignoring the server's reply.
    ///
    /// This saves you from having to write `let _ =` to silence warnings.
    pub async fn reply_only(&self, content: impl ToString) -> euphoxide::Result<()> {
        self.send_only(Some(self.msg.id), content).await
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

#[async_trait]
#[expect(unused_variables)]
pub trait Command<D = (), E = euphoxide::Error> {
    fn info(&self, ctx: &Context<D, E>) -> Info {
        Info::default()
    }

    async fn execute(&self, ctx: &Context<D, E>, arg: &str) -> Result<Propagate, E>;
}

pub trait CommandExt: Sized {
    fn described(self) -> Described<Self> {
        Described::new(self)
    }

    fn hidden(self) -> Described<Self> {
        Described::hidden(self)
    }

    fn prefixed(self, prefix: impl ToString) -> Prefixed<Self> {
        Prefixed::new(prefix, self)
    }

    fn global(self, name: impl ToString) -> Global<Self> {
        Global::new(name, self)
    }

    fn general(self, name: impl ToString) -> General<Self> {
        General::new(name, self)
    }

    fn specific(self, name: impl ToString) -> Specific<Self> {
        Specific::new(name, self)
    }

    #[cfg(feature = "clap")]
    fn clap(self) -> clap::Clap<Self> {
        clap::Clap(self)
    }

    fn add_to<D, E>(self, commands: &mut Commands<D, E>)
    where
        Self: Command<D, E> + Send + Sync + 'static,
    {
        commands.add(self);
    }
}

// Sadly this doesn't work: `impl<E, C: Command<E>> CommandExt for C {}` leaves
// E unconstrained. Instead, we just implement CommandExt for all types. This is
// fine since it'll crash and burn once we try to use the created commands as
// actual commands. It also follows the spirit of adding trait constraints only
// where they are necessary.
impl<C> CommandExt for C {}

pub struct Commands<D = (), E = euphoxide::Error> {
    commands: Vec<Box<dyn Command<D, E> + Sync + Send>>,
    data: D,
}

impl<D, E> Commands<D, E> {
    pub fn new(data: D) -> Self {
        Self {
            commands: vec![],
            data,
        }
    }

    /// User-supplied application data.
    pub fn data(&self) -> &D {
        &self.data
    }

    pub fn add(&mut self, command: impl Command<D, E> + Sync + Send + 'static) {
        self.commands.push(Box::new(command));
    }

    pub fn then(mut self, command: impl Command<D, E> + Sync + Send + 'static) -> Self {
        self.add(command);
        self
    }

    pub fn build(self) -> Arc<Self> {
        Arc::new(self)
    }

    pub fn infos(&self, ctx: &Context<D, E>) -> Vec<Info> {
        self.commands.iter().map(|c| c.info(ctx)).collect()
    }

    pub async fn handle_message(
        self: Arc<Self>,
        clients: Clients,
        client: Client,
        conn: ClientConnHandle,
        joined: Joined,
        msg: Message,
    ) -> Result<Propagate, E> {
        let ctx = Context {
            commands: self.clone(),
            clients,
            client,
            conn,
            joined,
            msg,
        };

        for command in &self.commands {
            let propagate = command.execute(&ctx, &ctx.msg.content).await?;
            if propagate == Propagate::No {
                return Ok(Propagate::No);
            }
        }

        Ok(Propagate::Yes)
    }

    pub async fn handle_event(
        self: Arc<Self>,
        clients: Clients,
        client: Client,
        event: ClientEvent,
    ) -> Result<Propagate, E> {
        let ClientEvent::Packet {
            conn,
            state,
            packet,
        } = event
        else {
            return Ok(Propagate::Yes);
        };

        let Ok(Data::SendEvent(SendEvent(msg))) = packet.content else {
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
impl<D: Default, E> Default for Commands<D, E> {
    fn default() -> Self {
        Self::new(Default::default())
    }
}
