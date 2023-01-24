use async_trait::async_trait;
use clap::Parser;

use crate::api::Message;
use crate::bot::command::{ClapCommand, Context};
use crate::bot::commands::CommandInfo;
use crate::conn;

/// Show full bot help.
#[derive(Parser)]
pub struct Args {}

pub struct FullHelp {
    pub before: String,
    pub after: String,
}

impl FullHelp {
    pub fn new<S1: ToString, S2: ToString>(before: S1, after: S2) -> Self {
        Self {
            before: before.to_string(),
            after: after.to_string(),
        }
    }
}

pub trait HasDescriptions {
    fn descriptions(&self) -> &[CommandInfo];
}

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
        let mut result = String::new();

        if !self.before.is_empty() {
            result.push_str(&self.before);
            result.push('\n');
        }

        for help in bot.descriptions() {
            if !help.visible {
                continue;
            }

            let usage = help.kind.usage(&help.name, &ctx.joined.session.name);
            let line = if let Some(description) = &help.description {
                format!("{usage} - {description}\n")
            } else {
                format!("{usage}\n")
            };

            result.push_str(&line);
        }

        if !self.after.is_empty() {
            result.push_str(&self.after);
            result.push('\n');
        }

        ctx.reply(msg.id, result).await?;
        Ok(())
    }
}
