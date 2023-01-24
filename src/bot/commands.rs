use std::collections::HashMap;

use crate::api::packet::ParsedPacket;
use crate::api::{Data, SendEvent};
use crate::conn;

use super::command::{Command, Context, Kind};
use super::instance::{InstanceConfig, Snapshot};

fn normalize_specific_nick(nick: &str) -> String {
    Kind::specific_nick(nick).to_lowercase()
}

/// Parse leading whitespace followed by an `!`-initiated command.
///
/// Returns the command name and the remaining text with one leading whitespace
/// removed. The remaining text may be the empty string.
fn parse_command(text: &str) -> Option<(&str, &str)> {
    let text = text.trim_start();
    let text = text.strip_prefix('!')?;
    let (name, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    if name.is_empty() {
        return None;
    }
    Some((name, rest))
}

/// Parse leading whitespace followed by an `@`-initiated nick.
///
/// Returns the nick and the remaining text with one leading whitespace removed.
/// The remaining text may be the empty string.
fn parse_specific(text: &str) -> Option<(&str, &str)> {
    let text = text.trim_start();
    let text = text.strip_prefix('@')?;
    let (name, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    if name.is_empty() {
        return None;
    }
    Some((name, rest))
}

pub struct CommandInfo {
    pub kind: Kind,
    pub name: String,
    pub description: Option<String>,
    pub visible: bool,
}

struct CommandWrapper<B> {
    command: Box<dyn Command<B>>,
    visible: bool,
}

pub struct Commands<B> {
    global: HashMap<String, CommandWrapper<B>>,
    general: HashMap<String, CommandWrapper<B>>,
    specific: HashMap<String, CommandWrapper<B>>,
}

impl<B> Commands<B> {
    /// Global commands always respond. They override any specific or general
    /// commands of the same name.
    ///
    /// Use this if your bot "owns" the command and no other bot uses it.
    pub fn global<S, C>(mut self, name: S, command: C, visible: bool) -> Self
    where
        S: ToString,
        C: Command<B> + 'static,
    {
        let command = Box::new(command);
        let info = CommandWrapper { command, visible };
        self.global.insert(name.to_string(), info);
        self
    }

    /// General commands only respond if no nick is specified.
    ///
    /// Use this if your or any other bot has a specific command of the same
    /// name.
    pub fn general<S, C>(mut self, name: S, command: C, visible: bool) -> Self
    where
        S: ToString,
        C: Command<B> + 'static,
    {
        let command = Box::new(command);
        let info = CommandWrapper { command, visible };
        self.general.insert(name.to_string(), info);
        self
    }

    /// Specific commands only respond if the bot's current nick is specified.
    pub fn specific<S, C>(mut self, name: S, command: C, visible: bool) -> Self
    where
        S: ToString,
        C: Command<B> + 'static,
    {
        let command = Box::new(command);
        let info = CommandWrapper { command, visible };
        self.specific.insert(name.to_string(), info);
        self
    }

    pub fn descriptions(&self) -> Vec<CommandInfo> {
        let mut keys = (self.global.keys())
            .chain(self.general.keys())
            .chain(self.specific.keys())
            .collect::<Vec<_>>();
        keys.sort_unstable();
        keys.dedup();

        let mut result = vec![];
        for name in keys {
            if let Some(wrapper) = self.global.get(name) {
                result.push(CommandInfo {
                    name: name.clone(),
                    kind: Kind::Global,
                    visible: wrapper.visible,
                    description: wrapper.command.description(),
                });
                continue; // Shadows general and specific commands
            }

            if let Some(wrapper) = self.general.get(name) {
                result.push(CommandInfo {
                    name: name.clone(),
                    kind: Kind::General,
                    visible: wrapper.visible,
                    description: wrapper.command.description(),
                });
            }

            if let Some(wrapper) = self.specific.get(name) {
                result.push(CommandInfo {
                    name: name.clone(),
                    kind: Kind::Specific,
                    visible: wrapper.visible,
                    description: wrapper.command.description(),
                });
            }
        }

        result
    }

    /// Returns `true` if a command was found and executed, `false` otherwise.
    pub async fn handle_packet(
        &self,
        config: &InstanceConfig,
        packet: &ParsedPacket,
        snapshot: &Snapshot,
        bot: &mut B,
    ) -> bool {
        let msg = match &packet.content {
            Ok(Data::SendEvent(SendEvent(msg))) => msg,
            _ => return false,
        };

        let joined = match &snapshot.state {
            conn::State::Joining(_) => return false,
            conn::State::Joined(joined) => joined.clone(),
        };

        let (cmd_name, rest) = match parse_command(&msg.content) {
            Some(parsed) => parsed,
            None => return false,
        };

        let mut ctx = Context {
            name: cmd_name.to_string(),
            kind: Kind::Global,
            config: config.clone(),
            conn_tx: snapshot.conn_tx.clone(),
            joined,
        };

        if let Some(wrapper) = self.global.get(cmd_name) {
            ctx.kind = Kind::Global;
            wrapper.command.execute(rest, msg, &ctx, bot).await;
            return true;
        }

        if let Some((cmd_nick, rest)) = parse_specific(rest) {
            if let Some(wrapper) = self.specific.get(cmd_name) {
                let nick_norm = normalize_specific_nick(&ctx.joined.session.name);
                let cmd_nick_norm = normalize_specific_nick(cmd_nick);
                if nick_norm == cmd_nick_norm {
                    ctx.kind = Kind::Specific;
                    wrapper.command.execute(rest, msg, &ctx, bot).await;
                    return true;
                }
            }

            // The command looks like a specific command. If we treated it like
            // a general command just because the nick doesn't match, we would
            // interpret other bots' specific commands as general commands.
            //
            // To call a specific command with a mention as its first positional
            // argument, -- can be used.
            return false;
        }

        if let Some(wrapper) = self.general.get(cmd_name) {
            ctx.kind = Kind::General;
            wrapper.command.execute(rest, msg, &ctx, bot).await;
            return true;
        }

        false
    }
}

#[cfg(test)]
mod test {
    use super::{parse_command, parse_specific};

    #[test]
    fn test_parse_command() {
        assert_eq!(parse_command("!foo"), Some(("foo", "")));
        assert_eq!(parse_command("    !foo"), Some(("foo", "")));
        assert_eq!(parse_command("!foo    "), Some(("foo", "   ")));
        assert_eq!(parse_command("    !foo    "), Some(("foo", "   ")));
        assert_eq!(parse_command("!foo @bar"), Some(("foo", "@bar")));
        assert_eq!(parse_command("!foo     @bar"), Some(("foo", "    @bar")));
        assert_eq!(parse_command("!foo @bar    "), Some(("foo", "@bar    ")));
        assert_eq!(parse_command("! foo @bar"), None);
        assert_eq!(parse_command("!"), None);
        assert_eq!(parse_command("?foo"), None);
    }

    #[test]
    fn test_parse_specific() {
        assert_eq!(parse_specific("@foo"), Some(("foo", "")));
        assert_eq!(parse_specific("    @foo"), Some(("foo", "")));
        assert_eq!(parse_specific("@foo    "), Some(("foo", "   ")));
        assert_eq!(parse_specific("    @foo    "), Some(("foo", "   ")));
        assert_eq!(parse_specific("@foo !bar"), Some(("foo", "!bar")));
        assert_eq!(parse_specific("@foo     !bar"), Some(("foo", "    !bar")));
        assert_eq!(parse_specific("@foo !bar    "), Some(("foo", "!bar    ")));
        assert_eq!(parse_specific("@ foo !bar"), None);
        assert_eq!(parse_specific("@"), None);
        assert_eq!(parse_specific("?foo"), None);
    }
}
