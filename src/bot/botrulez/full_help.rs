use async_trait::async_trait;
use clap::Parser;

use crate::api::Message;
use crate::bot::command::{ClapCommand, Command, Context};
use crate::conn;

pub struct FullHelp {
    pub before: String,
    pub after: String,
}

pub trait HasDescriptions {
    fn descriptions(&self, ctx: &Context) -> Vec<String>;
}

impl FullHelp {
    pub fn new<S1: ToString, S2: ToString>(before: S1, after: S2) -> Self {
        Self {
            before: before.to_string(),
            after: after.to_string(),
        }
    }

    fn formulate_reply<B: HasDescriptions>(&self, ctx: &Context, bot: &B) -> String {
        let mut result = String::new();

        if !self.before.is_empty() {
            result.push_str(&self.before);
            result.push('\n');
        }

        for description in bot.descriptions(ctx) {
            result.push_str(&description);
            result.push('\n');
        }

        if !self.after.is_empty() {
            result.push_str(&self.after);
            result.push('\n');
        }

        result
    }
}

#[async_trait]
impl<B, E> Command<B, E> for FullHelp
where
    B: HasDescriptions + Send,
    E: From<conn::Error>,
{
    async fn execute(&self, arg: &str, msg: &Message, ctx: &Context, bot: &mut B) -> Result<(), E> {
        if arg.trim().is_empty() {
            let reply = self.formulate_reply(ctx, bot);
            ctx.reply(msg.id, reply).await?;
        }
        Ok(())
    }
}

/// Show full bot help.
#[derive(Parser)]
pub struct Args {}

#[async_trait]
impl<B, E> ClapCommand<B, E> for FullHelp
where
    B: HasDescriptions + Send,
    E: From<conn::Error>,
{
    type Args = Args;

    async fn execute(
        &self,
        _args: Self::Args,
        msg: &Message,
        ctx: &Context,
        bot: &mut B,
    ) -> Result<(), E> {
        let reply = self.formulate_reply(ctx, bot);
        ctx.reply(msg.id, reply).await?;
        Ok(())
    }
}
