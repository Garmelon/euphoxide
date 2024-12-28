use async_trait::async_trait;
#[cfg(feature = "clap")]
use clap::Parser;
use euphoxide::api::Message;

#[cfg(feature = "clap")]
use crate::command::clap::ClapCommand;
use crate::command::{Command, Context, Propagate};

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
impl<E> Command<E> for Ping
where
    E: From<euphoxide::Error>,
{
    async fn execute(&self, arg: &str, msg: &Message, ctx: &Context<E>) -> Result<Propagate, E> {
        if arg.trim().is_empty() {
            ctx.reply_only(msg.id, &self.0).await?;
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
impl<E> ClapCommand<E> for Ping
where
    E: From<euphoxide::Error>,
{
    type Args = PingArgs;

    async fn execute(
        &self,
        _args: Self::Args,
        msg: &Message,
        ctx: &Context<E>,
    ) -> Result<Propagate, E> {
        ctx.reply_only(msg.id, &self.0).await?;
        Ok(Propagate::No)
    }
}
