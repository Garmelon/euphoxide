mod clap;

use std::future::Future;

use async_trait::async_trait;

use crate::api::{self, Message, MessageId};
use crate::conn::{self, ConnTx, Joined};

pub use self::clap::{Clap, ClapCommand};

use super::instance::InstanceConfig;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Global commands always respond. They override any specific or general
    /// commands of the same name.
    Global,
    /// General commands only respond if no nick is specified.
    General,
    /// Specific commands only respond if the bot's current nick is specified.
    Specific,
}

impl Kind {
    pub fn global_and_general_usage(cmd_name: &str) -> String {
        format!("!{cmd_name}")
    }

    pub fn specific_nick(nick: &str) -> String {
        nick.replace(char::is_whitespace, "")
    }

    pub fn specific_usage(cmd_name: &str, nick: &str) -> String {
        format!("!{cmd_name} @{}", Self::specific_nick(nick))
    }

    pub fn usage(self, cmd_name: &str, nick: &str) -> String {
        match self {
            Self::Global | Self::General => Self::global_and_general_usage(cmd_name),
            Self::Specific => Self::specific_usage(cmd_name, nick),
        }
    }
}

pub struct Context {
    pub name: String,
    pub kind: Kind,
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

#[async_trait]
pub trait Command<B, E> {
    fn description(&self) -> Option<String> {
        None
    }

    async fn execute(&self, arg: &str, msg: &Message, ctx: &Context, bot: &mut B) -> Result<(), E>;
}
