use async_trait::async_trait;
#[cfg(feature = "clap")]
use clap::Parser;

#[cfg(feature = "clap")]
use crate::clap::ClapCommand;
use crate::{Command, Context, Propagate};

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
impl<D, E> Command<D, E> for Ping
where
    D: Send + Sync,
    E: From<euphoxide::Error>,
{
    async fn execute(&self, arg: &str, ctx: &Context<D, E>) -> Result<Propagate, E> {
        if arg.trim().is_empty() {
            ctx.reply_only(&self.0).await?;
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

    async fn execute(&self, _args: Self::Args, ctx: &Context<D, E>) -> Result<Propagate, E> {
        ctx.reply_only(&self.0).await?;
        Ok(Propagate::No)
    }
}
