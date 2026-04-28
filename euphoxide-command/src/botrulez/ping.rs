use async_trait::async_trait;
#[cfg(feature = "clap")]
use clap::Parser;

#[cfg(feature = "clap")]
use crate::clap::ClapCommand;
use crate::{Command, Context, Propagate};

/// Simple ping reply.
///
/// Combine with [`crate::General`] for a `!ping` command and with
/// [`crate::Specific`] for a `!ping @BotName` command. See also
/// <https://github.com/jedevc/botrulez#ping>.
pub struct Ping(pub String);

impl Ping {
    /// Create a ping reply with a specific message.
    ///
    /// Use [`Self::default()`] for a generic `Pong!` reply.
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
impl<D, E> Command<D, E> for Ping
where
    D: Send + Sync,
    E: From<euphoxide::Error>,
{
    async fn execute(&self, ctx: &Context<D, E>, arg: &str) -> Result<Propagate, E> {
        if arg.trim().is_empty() {
            ctx.reply_only(&self.0)?;
            Ok(Propagate::No)
        } else {
            Ok(Propagate::Yes)
        }
    }
}

/// Trigger a short reply.
#[cfg(feature = "clap")]
#[derive(Parser)]
pub struct PingArgs {}

#[cfg(feature = "clap")]
#[async_trait]
impl<D, E> ClapCommand<D, E> for Ping
where
    D: Send + Sync,
    E: From<euphoxide::Error>,
{
    type Args = PingArgs;

    async fn execute(&self, ctx: &Context<D, E>, _args: Self::Args) -> Result<Propagate, E> {
        ctx.reply_only(&self.0)?;
        Ok(Propagate::No)
    }
}
