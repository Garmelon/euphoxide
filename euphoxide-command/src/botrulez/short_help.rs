use async_trait::async_trait;
#[cfg(feature = "clap")]
use clap::Parser;

#[cfg(feature = "clap")]
use crate::clap::ClapCommand;
use crate::{Command, Context, Propagate};

pub struct ShortHelp(pub String);

impl ShortHelp {
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
    async fn execute(&self, arg: &str, ctx: &Context<D, E>) -> Result<Propagate, E> {
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

    async fn execute(&self, _args: Self::Args, ctx: &Context<D, E>) -> Result<Propagate, E> {
        ctx.reply_only(&self.0).await?;
        Ok(Propagate::No)
    }
}
