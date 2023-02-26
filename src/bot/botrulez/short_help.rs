use async_trait::async_trait;
use clap::Parser;

use crate::api::Message;
use crate::bot::command::{ClapCommand, Command, Context};
use crate::conn;

pub struct ShortHelp(pub String);

impl ShortHelp {
    pub fn new<S: ToString>(text: S) -> Self {
        Self(text.to_string())
    }
}

#[async_trait]
impl<B, E> Command<B, E> for ShortHelp
where
    E: From<conn::Error>,
{
    async fn execute(
        &self,
        arg: &str,
        msg: &Message,
        ctx: &Context,
        _bot: &mut B,
    ) -> Result<(), E> {
        if arg.trim().is_empty() {
            ctx.reply(msg.id, &self.0).await?;
        }
        Ok(())
    }
}

/// Show short bot help.
#[derive(Parser)]
pub struct Args {}

#[async_trait]
impl<B, E> ClapCommand<B, E> for ShortHelp
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
