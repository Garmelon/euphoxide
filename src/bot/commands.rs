use crate::api::packet::ParsedPacket;
use crate::api::{Data, SendEvent};
use crate::conn;

use super::command::{Command, Context};
use super::instance::{InstanceConfig, Snapshot};

pub struct Commands<B, E> {
    commands: Vec<Box<dyn Command<B, E> + Send + Sync>>,
    fallthrough: bool,
}

impl<B, E> Commands<B, E> {
    pub fn new() -> Self {
        Self {
            commands: vec![],
            fallthrough: false,
        }
    }

    /// Whether further commands should be executed after a command returns
    /// `true`.
    ///
    /// If disabled, commands are run until the first command that returns
    /// `true`. If enabled, all commands are run irrespective of their return
    /// values.
    pub fn fallthrough(&self) -> bool {
        self.fallthrough
    }

    /// Set whether fallthrough is active.
    ///
    /// See [`Self::fallthrough`] for more details.
    pub fn set_fallthrough(&mut self, active: bool) {
        self.fallthrough = active;
    }

    pub fn add<C>(&mut self, command: C)
    where
        C: Command<B, E> + Send + Sync + 'static,
    {
        self.commands.push(Box::new(command));
    }

    pub fn descriptions(&self, ctx: &Context) -> Vec<String> {
        self.commands
            .iter()
            .filter_map(|c| c.description(ctx))
            .collect::<Vec<_>>()
    }

    /// Returns `true` if one or more commands returned `true`, `false`
    /// otherwise.
    pub async fn handle_packet(
        &self,
        config: &InstanceConfig,
        packet: &ParsedPacket,
        snapshot: &Snapshot,
        bot: &mut B,
    ) -> Result<bool, E> {
        let msg = match &packet.content {
            Ok(Data::SendEvent(SendEvent(msg))) => msg,
            _ => return Ok(false),
        };

        let joined = match &snapshot.state {
            conn::State::Joining(_) => return Ok(false),
            conn::State::Joined(joined) => joined.clone(),
        };

        let ctx = Context {
            config: config.clone(),
            conn_tx: snapshot.conn_tx.clone(),
            joined,
        };

        let mut handled = false;
        for command in &self.commands {
            handled = handled || command.execute(&msg.content, msg, &ctx, bot).await?;
            if !self.fallthrough && handled {
                break;
            }
        }

        Ok(handled)
    }
}

impl<B, E> Default for Commands<B, E> {
    fn default() -> Self {
        Self::new()
    }
}
