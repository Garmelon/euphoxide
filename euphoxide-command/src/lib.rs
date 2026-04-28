//! A bot command framework for `euphoxide`.
//!
//! It is considered experimental for now.

#![warn(missing_docs)]

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

use crate::basic::{Exact, Hidden};

use self::{
    bang::{General, Global, Specific},
    basic::{Described, Prefixed},
};

/// Execution context for commands.
///
/// See [`Command`] for more details on the type parameters.
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
    pub fn send(
        &self,
        parent: Option<MessageId>,
        content: impl ToString,
    ) -> euphoxide::Result<impl Future<Output = euphoxide::Result<SendReply>>> {
        self.conn.send(api::Send {
            content: content.to_string(),
            parent,
        })
    }

    /// Like [`Self::send`], but ignoring the server's reply.
    ///
    /// This saves you from having to write `let _ =` to silence warnings.
    pub fn send_only(
        &self,
        parent: Option<MessageId>,
        content: impl ToString,
    ) -> euphoxide::Result<()> {
        let _ignore = self.send(parent, content)?;
        Ok(())
    }

    /// Send a reply to the message that triggered the command.
    pub fn reply(
        &self,
        content: impl ToString,
    ) -> euphoxide::Result<impl Future<Output = euphoxide::Result<SendReply>>> {
        self.send(Some(self.msg.id), content)
    }

    /// Like [`Self::reply`], but ignoring the server's reply.
    ///
    /// This saves you from having to write `let _ =` to silence warnings.
    pub fn reply_only(&self, content: impl ToString) -> euphoxide::Result<()> {
        self.send_only(Some(self.msg.id), content)
    }
}

/// Information used to render a command's help.
#[derive(Default)]
pub struct CommandHelp {
    /// How to trigger the command, e.g. `!foo`.
    pub trigger: Option<String>,
    /// What happens when the command is triggered.
    pub description: Option<String>,
}

impl CommandHelp {
    /// Create an empty command help.
    ///
    /// Alias for [`Self::default`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Prepend a trigger to the existing trigger.
    ///
    /// Adds a single space between the triggers if necessary.
    pub fn prepend_trigger(&mut self, trigger: impl ToString) {
        let cur_trigger = self.trigger.get_or_insert_default();
        if !cur_trigger.is_empty() {
            cur_trigger.insert(0, ' ');
        }
        cur_trigger.insert_str(0, &trigger.to_string());
    }
}

/// Whether a message should propagate to subsequent commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Propagate {
    /// The message should not propagate.
    No,
    /// The message should propagate.
    Yes,
}

/// A bot command.
///
/// This trait is not restricted to the traditional bang commands. Instead, it
/// can be used to react to any kind of message.
///
/// [`Command<D, E>`] has two type parameters:
/// - `D` is user-supplied application data passed along in the [`Context`].
/// - `E` is the custom error type returned by [`Command::execute`].
#[async_trait]
#[expect(unused_variables)]
pub trait Command<D = (), E = euphoxide::Error> {
    /// Get help information for this command.
    fn help(&self, ctx: &Context<D, E>) -> CommandHelp {
        CommandHelp::default()
    }

    /// Execute this command.
    ///
    /// This method is called for every message in a room, assuming no earlier
    /// command returned [`Propagate::No`] for the same message. It is not
    /// called for messages sent by the bot itself in the current session.
    async fn execute(&self, ctx: &Context<D, E>, arg: &str) -> Result<Propagate, E>;
}

#[async_trait]
impl<D, E, C> Command<D, E> for &C
where
    D: Send + Sync,
    C: Command<D, E> + Sync,
{
    fn help(&self, ctx: &Context<D, E>) -> CommandHelp {
        (*self).help(ctx)
    }

    async fn execute(&self, ctx: &Context<D, E>, arg: &str) -> Result<Propagate, E> {
        (*self).execute(ctx, arg).await
    }
}

/// Helper trait for constructing [`Command`]s with a function chaining API.
pub trait CommandExt: Sized {
    /// Wrap the command in an [`Exact`].
    fn exact(self) -> Exact<Self> {
        Exact(self)
    }

    /// Wrap the command in a [`Hidden`].
    fn hidden(self) -> Hidden<Self> {
        Hidden(self)
    }

    /// Wrap the command in a [`Described`].
    fn described(self, description: impl ToString) -> Described<Self> {
        Described::new(self, description)
    }

    /// Wrap the command in a [`Prefixed`].
    fn prefixed(self, prefix: impl ToString) -> Prefixed<Self> {
        Prefixed::new(prefix, self)
    }

    /// Wrap the command in a [`Global`].
    fn global(self, name: impl ToString) -> Global<Self> {
        Global::new(name, self)
    }

    /// Wrap the command in a [`General`].
    fn general(self, name: impl ToString) -> General<Self> {
        General::new(name, self)
    }

    /// Wrap the command in a [`Specific`].
    fn specific(self, name: impl ToString) -> Specific<Self> {
        Specific::new(name, self)
    }

    /// Wrap the command in a [`clap::Clap`].
    #[cfg(feature = "clap")]
    fn clap(self) -> clap::Clap<Self> {
        clap::Clap(self)
    }

    /// Register the command with a [`Commands`].
    ///
    /// See also [`Commands::add`].
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

/// A collection of [`Command`]s.
///
/// Holds an ordered list of commands, as well as user-supplied application data
/// that is passed along to the commands in the [`Context`].
///
/// See [`Command`] for more details on the type parameters.
pub struct Commands<D = (), E = euphoxide::Error> {
    commands: Vec<Box<dyn Command<D, E> + Sync + Send>>,
    data: D,
}

impl<D, E> Commands<D, E> {
    /// Create an empty command collection with user-supplied application data.
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

    /// Append a command to the command list.
    pub fn add(&mut self, command: impl Command<D, E> + Sync + Send + 'static) {
        self.commands.push(Box::new(command));
    }

    /// Get help information for all commands.
    pub fn help(&self, ctx: &Context<D, E>) -> Vec<CommandHelp> {
        self.commands.iter().map(|c| c.help(ctx)).collect()
    }

    /// Execute stored commands for a [`Message`] that you've just received.
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

    /// Execute stored commands for a [`ClientEvent`] that you've just received.
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
