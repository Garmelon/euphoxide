use async_trait::async_trait;
use euphoxide::api::Message;

use super::{Command, Context, Info, Propagate};

pub struct Hidden<C> {
    pub inner: C,
    pub allow_trigger: bool,
    pub allow_description: bool,
}

impl<C> Hidden<C> {
    pub fn new(inner: C) -> Self {
        Self {
            inner,
            allow_trigger: false,
            allow_description: false,
        }
    }

    pub fn with_allow_trigger(mut self, allow: bool) -> Self {
        self.allow_trigger = allow;
        self
    }

    pub fn with_allow_description(mut self, allow: bool) -> Self {
        self.allow_description = allow;
        self
    }
}

#[async_trait]
impl<B, E, C> Command<B, E> for Hidden<C>
where
    B: Send,
    C: Command<B, E> + Send + Sync,
{
    fn info(&self, ctx: &Context) -> Info {
        let info = self.inner.info(ctx);
        Info {
            trigger: info.trigger.filter(|_| self.allow_trigger),
            description: info.description.filter(|_| self.allow_description),
        }
    }

    async fn execute(
        &self,
        arg: &str,
        msg: &Message,
        ctx: &Context,
        bot: &mut B,
    ) -> Result<Propagate, E> {
        self.inner.execute(arg, msg, ctx, bot).await
    }
}
