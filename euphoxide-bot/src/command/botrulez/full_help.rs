use async_trait::async_trait;
#[cfg(feature = "clap")]
use clap::Parser;
use euphoxide::api::Message;

#[cfg(feature = "clap")]
use crate::command::clap::ClapCommand;
use crate::command::{Command, Context, Propagate};

#[derive(Default)]
pub struct FullHelp {
    pub before: String,
    pub after: String,
}

impl FullHelp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_before(mut self, before: impl ToString) -> Self {
        self.before = before.to_string();
        self
    }

    pub fn with_after(mut self, after: impl ToString) -> Self {
        self.after = after.to_string();
        self
    }

    fn formulate_reply<E>(&self, ctx: &Context<E>) -> String {
        let mut result = String::new();

        if !self.before.is_empty() {
            result.push_str(&self.before);
            result.push('\n');
        }

        for info in ctx.commands.infos(ctx) {
            if let Some(trigger) = &info.trigger {
                result.push_str(trigger);
                if let Some(description) = &info.description {
                    result.push_str(" - ");
                    result.push_str(description);
                }
                result.push('\n');
            }
        }

        if !self.after.is_empty() {
            result.push_str(&self.after);
            result.push('\n');
        }

        result
    }
}

#[async_trait]
impl<E> Command<E> for FullHelp
where
    E: From<euphoxide::Error>,
{
    async fn execute(&self, arg: &str, msg: &Message, ctx: &Context<E>) -> Result<Propagate, E> {
        if arg.trim().is_empty() {
            let reply = self.formulate_reply(ctx);
            ctx.reply_only(msg.id, reply).await?;
            Ok(Propagate::No)
        } else {
            Ok(Propagate::Yes)
        }
    }
}

/// Show full bot help.
#[cfg(feature = "clap")]
#[derive(Parser)]
pub struct FullHelpArgs {}

#[cfg(feature = "clap")]
#[async_trait]
impl<E> ClapCommand<E> for FullHelp
where
    E: From<euphoxide::Error>,
{
    type Args = FullHelpArgs;

    async fn execute(
        &self,
        _args: Self::Args,
        msg: &Message,
        ctx: &Context<E>,
    ) -> Result<Propagate, E> {
        let reply = self.formulate_reply(ctx);
        ctx.reply_only(msg.id, reply).await?;
        Ok(Propagate::No)
    }
}
