//! Basic command wrappers.

use async_trait::async_trait;
use euphoxide::api::Message;

use crate::bot::Bot;

use super::{Command, Context, Info, Propagate};

/// Rewrite or hide command info.
pub struct Described<C> {
    pub inner: C,
    pub trigger: Option<Option<String>>,
    pub description: Option<Option<String>>,
}

impl<C> Described<C> {
    pub fn new(inner: C) -> Self {
        Self {
            inner,
            trigger: None,
            description: None,
        }
    }

    pub fn hidden(inner: C) -> Self {
        Self::new(inner)
            .with_trigger_hidden()
            .with_description_hidden()
    }

    pub fn with_trigger(mut self, trigger: impl ToString) -> Self {
        self.trigger = Some(Some(trigger.to_string()));
        self
    }

    pub fn with_trigger_hidden(mut self) -> Self {
        self.trigger = Some(None);
        self
    }

    pub fn with_description(mut self, description: impl ToString) -> Self {
        self.description = Some(Some(description.to_string()));
        self
    }

    pub fn with_description_hidden(mut self) -> Self {
        self.description = Some(None);
        self
    }
}

#[async_trait]
impl<S, E, C> Command<S, E> for Described<C>
where
    S: Send + Sync,
    C: Command<S, E> + Sync,
{
    fn info(&self, ctx: &Context) -> Info {
        let info = self.inner.info(ctx);
        Info {
            trigger: self.trigger.clone().unwrap_or(info.trigger),
            description: self.description.clone().unwrap_or(info.description),
        }
    }

    async fn execute(
        &self,
        arg: &str,
        msg: &Message,
        ctx: &Context,
        bot: &Bot<S, E>,
    ) -> Result<Propagate, E> {
        self.inner.execute(arg, msg, ctx, bot).await
    }
}

pub struct Prefixed<C> {
    prefix: String,
    inner: C,
}

impl<C> Prefixed<C> {
    pub fn new<S: ToString>(prefix: S, inner: C) -> Self {
        Self {
            prefix: prefix.to_string(),
            inner,
        }
    }
}

#[async_trait]
impl<S, E, C> Command<S, E> for Prefixed<C>
where
    S: Send + Sync,
    C: Command<S, E> + Sync,
{
    fn info(&self, ctx: &Context) -> Info {
        self.inner.info(ctx).with_prepended_trigger(&self.prefix)
    }

    async fn execute(
        &self,
        arg: &str,
        msg: &Message,
        ctx: &Context,
        bot: &Bot<S, E>,
    ) -> Result<Propagate, E> {
        if let Some(rest) = arg.trim_start().strip_prefix(&self.prefix) {
            self.inner.execute(rest, msg, ctx, bot).await
        } else {
            Ok(Propagate::Yes)
        }
    }
}
