use async_trait::async_trait;

use crate::api::Message;

use super::{Command, Context};

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
impl<B, E, C> Command<B, E> for Prefixed<C>
where
    B: Send,
    C: Command<B, E> + Send + Sync,
{
    fn description(&self, ctx: &Context) -> Option<String> {
        let inner = self.inner.description(ctx)?;
        Some(format!("{} - {inner}", self.prefix))
    }

    async fn execute(
        &self,
        arg: &str,
        msg: &Message,
        ctx: &Context,
        bot: &mut B,
    ) -> Result<bool, E> {
        if let Some(rest) = arg.trim_start().strip_prefix(&self.prefix) {
            self.inner.execute(rest, msg, ctx, bot).await
        } else {
            Ok(false)
        }
    }
}
