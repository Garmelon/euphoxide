mod bang;
mod clap;
mod hidden;
mod prefixed;

use std::future::Future;

use async_trait::async_trait;

use crate::api::{self, Message, MessageId};
use crate::conn::{self, ConnTx, Joined};

pub use self::bang::*;
pub use self::clap::*;
pub use self::hidden::*;
pub use self::prefixed::*;

use super::instance::InstanceConfig;

pub struct Context {
    pub config: InstanceConfig,
    pub conn_tx: ConnTx,
    pub joined: Joined,
}

impl Context {
    pub fn send<S: ToString>(&self, content: S) -> impl Future<Output = conn::Result<Message>> {
        let cmd = api::Send {
            content: content.to_string(),
            parent: None,
        };
        let reply = self.conn_tx.send(cmd);
        async move { reply.await.map(|r| r.0) }
    }

    pub fn reply<S: ToString>(
        &self,
        parent: MessageId,
        content: S,
    ) -> impl Future<Output = conn::Result<Message>> {
        let cmd = api::Send {
            content: content.to_string(),
            parent: Some(parent),
        };
        let reply = self.conn_tx.send(cmd);
        async move { reply.await.map(|r| r.0) }
    }
}

#[allow(unused_variables)]
#[async_trait]
pub trait Command<B, E> {
    fn description(&self, ctx: &Context) -> Option<String> {
        None
    }

    async fn execute(
        &self,
        arg: &str,
        msg: &Message,
        ctx: &Context,
        bot: &mut B,
    ) -> Result<bool, E>;
}
