use async_trait::async_trait;

use crate::api::{self, Message, MessageId};
use crate::conn::{self, ConnTx, Joined};

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
    pub kind: Kind,
    pub config: InstanceConfig,
    pub conn_tx: ConnTx,
    pub joined: Joined,
}

impl Context {
    pub async fn send<S: ToString>(&self, content: S) -> Result<Message, conn::Error> {
        let cmd = api::Send {
            content: content.to_string(),
            parent: None,
        };
        Ok(self.conn_tx.send(cmd).await?.0)
    }

    pub async fn reply<S: ToString>(
        &self,
        parent: MessageId,
        content: S,
    ) -> Result<Message, conn::Error> {
        let cmd = api::Send {
            content: content.to_string(),
            parent: Some(parent),
        };
        Ok(self.conn_tx.send(cmd).await?.0)
    }
}

#[async_trait]
pub trait Command<B> {
    fn description(&self) -> Option<String> {
        None
    }

    async fn execute(&self, arg: &str, msg: &Message, ctx: &Context, bot: &mut B);
}
