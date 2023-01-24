use async_trait::async_trait;
use clap::Parser;

use crate::api::Message;
use crate::bot::command::{ClapCommand, Context};
use crate::conn;

/// Trigger a short reply.
#[derive(Parser)]
pub struct Args {}

pub struct Ping(pub String);

impl Ping {
    pub fn new<S: ToString>(reply: S) -> Self {
        Self(reply.to_string())
    }
}

impl Default for Ping {
    fn default() -> Self {
        Self::new("Pong!")
    }
}

#[async_trait]
impl<B, E> ClapCommand<B, E> for Ping
where
    E: From<conn::Error>,
{
    type Args = Args;

    async fn execute(
        &self,
        _args: Self::Args,
        msg: &Message,
        ctx: &Context,
        _bot: &mut B,
    ) -> Result<(), E> {
        ctx.reply(msg.id, &self.0).await?;
        Ok(())
    }
}
