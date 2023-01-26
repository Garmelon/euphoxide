use crate::api::packet::ParsedPacket;
use crate::api::{Data, SendEvent};
use crate::conn;

use super::command::{Command, Context};
use super::instance::{InstanceConfig, Snapshot};

pub struct Commands<B, E> {
    commands: Vec<Box<dyn Command<B, E> + Send + Sync>>,
}

impl<B, E> Commands<B, E> {
    pub fn new() -> Self {
        Self { commands: vec![] }
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

    /// Returns `true` if a command was found and executed, `false` otherwise.
    pub async fn handle_packet(
        &self,
        config: &InstanceConfig,
        packet: &ParsedPacket,
        snapshot: &Snapshot,
        bot: &mut B,
    ) -> Result<(), E> {
        let msg = match &packet.content {
            Ok(Data::SendEvent(SendEvent(msg))) => msg,
            _ => return Ok(()),
        };

        let joined = match &snapshot.state {
            conn::State::Joining(_) => return Ok(()),
            conn::State::Joined(joined) => joined.clone(),
        };

        let ctx = Context {
            config: config.clone(),
            conn_tx: snapshot.conn_tx.clone(),
            joined,
        };

        for command in &self.commands {
            command.execute(&msg.content, msg, &ctx, bot).await?;
        }

        Ok(())
    }
}

impl<B, E> Default for Commands<B, E> {
    fn default() -> Self {
        Self::new()
    }
}
