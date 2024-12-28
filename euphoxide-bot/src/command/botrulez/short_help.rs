use async_trait::async_trait;
#[cfg(feature = "clap")]
use clap::Parser;
use euphoxide::api::Message;

#[cfg(feature = "clap")]
use crate::command::clap::ClapCommand;
use crate::{
    bot::Bot,
    command::{Command, Context, Propagate},
};

pub struct ShortHelp(pub String);

impl ShortHelp {
    pub fn new<S: ToString>(text: S) -> Self {
        Self(text.to_string())
    }
}

#[async_trait]
impl<E> Command<E> for ShortHelp
where
    E: From<euphoxide::Error>,
{
    async fn execute(
        &self,
        arg: &str,
        msg: &Message,
        ctx: &Context,
        _bot: &Bot<E>,
    ) -> Result<Propagate, E> {
        if arg.trim().is_empty() {
            ctx.reply_only(msg.id, &self.0).await?;
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
impl<E> ClapCommand<E> for ShortHelp
where
    E: From<euphoxide::Error>,
{
    type Args = ShortHelpArgs;

    async fn execute(
        &self,
        _args: Self::Args,
        msg: &Message,
        ctx: &Context,
        _bot: &Bot<E>,
    ) -> Result<Propagate, E> {
        ctx.reply_only(msg.id, &self.0).await?;
        Ok(Propagate::No)
    }
}
