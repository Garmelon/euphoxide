use async_trait::async_trait;

use crate::api::Message;

use super::{Command, Context};

pub struct Hidden<C>(pub C);

#[async_trait]
impl<B, E, C> Command<B, E> for Hidden<C>
where
    B: Send,
    C: Command<B, E> + Send + Sync,
{
    fn description(&self, _ctx: &Context) -> Option<String> {
        // Default implementation, repeated here for emphasis.
        None
    }

    async fn execute(
        &self,
        arg: &str,
        msg: &Message,
        ctx: &Context,
        bot: &mut B,
    ) -> Result<bool, E> {
        self.0.execute(arg, msg, ctx, bot).await
    }
}
