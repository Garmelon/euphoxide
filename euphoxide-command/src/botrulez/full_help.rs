use async_trait::async_trait;
#[cfg(feature = "clap")]
use clap::Parser;

#[cfg(feature = "clap")]
use crate::clap::ClapCommand;
use crate::{Command, Context, Propagate};

/// Detailed help reply listing all available commands.
///
/// Combine with [`crate::Specific`] for a `!help @BotName` command. See also
/// <https://github.com/jedevc/botrulez#help>.
#[derive(Default)]
pub struct FullHelp {
    /// Text to put before the command listing.
    pub before: String,
    /// Text to put after the command listing.
    pub after: String,
}

impl FullHelp {
    /// Create a [`FullHelp`] without additional text.
    ///
    /// Alias for [`FullHelp::default`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Set text to put before the command listing.
    pub fn with_before(mut self, before: impl ToString) -> Self {
        self.before = before.to_string();
        self
    }

    /// Set text to put after the command listing.
    pub fn with_after(mut self, after: impl ToString) -> Self {
        self.after = after.to_string();
        self
    }

    fn formulate_reply<D, E>(&self, ctx: &Context<D, E>) -> String {
        let mut result = String::new();

        if !self.before.is_empty() {
            result.push_str(&self.before);
            result.push('\n');
        }

        for help in ctx.commands.help(ctx) {
            if let Some(trigger) = &help.trigger {
                result.push_str(trigger);
                if let Some(description) = &help.description {
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
impl<D, E> Command<D, E> for FullHelp
where
    D: Send + Sync,
    E: From<euphoxide::Error>,
{
    async fn execute(&self, ctx: &Context<D, E>, arg: &str) -> Result<Propagate, E> {
        if arg.trim().is_empty() {
            let reply = self.formulate_reply(ctx);
            ctx.reply_only(reply)?;
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
impl<D, E> ClapCommand<D, E> for FullHelp
where
    D: Send + Sync,
    E: From<euphoxide::Error>,
{
    type Args = FullHelpArgs;

    async fn execute(&self, ctx: &Context<D, E>, _args: Self::Args) -> Result<Propagate, E> {
        let reply = self.formulate_reply(ctx);
        ctx.reply_only(reply)?;
        Ok(Propagate::No)
    }
}
