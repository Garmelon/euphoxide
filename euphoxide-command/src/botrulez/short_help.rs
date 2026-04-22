use async_trait::async_trait;
#[cfg(feature = "clap")]
use clap::Parser;

#[cfg(feature = "clap")]
use crate::clap::ClapCommand;
use crate::{Command, Context, Propagate};

/// Short help reply.
///
/// Combine with [`crate::General`] for a `!help` command. See also
/// <https://github.com/jedevc/botrulez#help>.
pub struct ShortHelp(
    /// Text to reply with.
    pub String,
);

impl ShortHelp {
    /// Create a [`ShortHelp`].
    pub fn new<S: ToString>(text: S) -> Self {
        Self(text.to_string())
    }
}

#[async_trait]
impl<D, E> Command<D, E> for ShortHelp
where
    D: Send + Sync,
    E: From<euphoxide::Error>,
{
    async fn execute(&self, ctx: &Context<D, E>, arg: &str) -> Result<Propagate, E> {
        if arg.trim().is_empty() {
            ctx.reply_only(&self.0).await?;
            Ok(Propagate::No)
        } else {
            Ok(Propagate::Yes)
        }
    }
}

/// Show short bot help.
#[cfg(feature = "clap")]
#[derive(Parser)]
pub struct ShortHelpArgs {}

#[cfg(feature = "clap")]
#[async_trait]
impl<D, E> ClapCommand<D, E> for ShortHelp
where
    D: Send + Sync,
    E: From<euphoxide::Error>,
{
    type Args = ShortHelpArgs;

    async fn execute(&self, ctx: &Context<D, E>, _args: Self::Args) -> Result<Propagate, E> {
        ctx.reply_only(&self.0).await?;
        Ok(Propagate::No)
    }
}
