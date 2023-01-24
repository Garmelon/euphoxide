use async_trait::async_trait;
use clap::Parser;

use crate::api::Message;
use crate::bot::command::{ClapCommand, Context};
use crate::conn;

/// Show short bot help.
#[derive(Parser)]
pub struct Args {}

pub struct ShortHelp(pub String);

impl ShortHelp {
    pub fn new<S: ToString>(text: S) -> Self {
        Self(text.to_string())
    }
}

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
