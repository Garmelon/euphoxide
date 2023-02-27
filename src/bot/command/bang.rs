use async_trait::async_trait;

use crate::api::Message;
use crate::nick;

use super::{Command, Context};

/// Parse leading whitespace followed by an prefix-initiated command.
///
/// Returns the command name and the remaining text with one leading whitespace
/// removed. The remaining text may be the empty string.
pub fn parse_prefix_initiated<'a>(text: &'a str, prefix: &str) -> Option<(&'a str, &'a str)> {
    let text = text.trim_start();
    let text = text.strip_prefix(prefix)?;
    let (name, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    if name.is_empty() {
        return None;
    }
    Some((name, rest))
}

pub struct Global<C> {
    prefix: String,
    name: String,
    inner: C,
}

impl<C> Global<C> {
    pub fn new<S: ToString>(name: S, inner: C) -> Self {
        Self {
            prefix: "!".to_string(),
            name: name.to_string(),
            inner,
        }
    }

    pub fn prefix<S: ToString>(mut self, prefix: S) -> Self {
        self.prefix = prefix.to_string();
        self
    }
}

#[async_trait]
impl<B, E, C> Command<B, E> for Global<C>
where
    B: Send,
    C: Command<B, E> + Send + Sync,
{
    fn description(&self, ctx: &Context) -> Option<String> {
        let inner = self.inner.description(ctx)?;
        Some(format!("{}{} - {inner}", self.prefix, self.name))
    }

    async fn execute(
        &self,
        arg: &str,
        msg: &Message,
        ctx: &Context,
        bot: &mut B,
    ) -> Result<bool, E> {
        // TODO Replace with let-else
        let (name, rest) = match parse_prefix_initiated(arg, &self.prefix) {
            Some(parsed) => parsed,
            None => return Ok(false),
        };

        if name != self.name {
            return Ok(false);
        }

        self.inner.execute(rest, msg, ctx, bot).await
    }
}

pub struct General<C> {
    prefix: String,
    name: String,
    inner: C,
}

impl<C> General<C> {
    pub fn new<S: ToString>(name: S, inner: C) -> Self {
        Self {
            prefix: "!".to_string(),
            name: name.to_string(),
            inner,
        }
    }

    pub fn prefix<S: ToString>(mut self, prefix: S) -> Self {
        self.prefix = prefix.to_string();
        self
    }
}

#[async_trait]
impl<B, E, C> Command<B, E> for General<C>
where
    B: Send,
    C: Command<B, E> + Send + Sync,
{
    fn description(&self, ctx: &Context) -> Option<String> {
        let inner = self.inner.description(ctx)?;
        Some(format!("{}{} - {inner}", self.prefix, self.name))
    }

    async fn execute(
        &self,
        arg: &str,
        msg: &Message,
        ctx: &Context,
        bot: &mut B,
    ) -> Result<bool, E> {
        // TODO Replace with let-else
        let (name, rest) = match parse_prefix_initiated(arg, &self.prefix) {
            Some(parsed) => parsed,
            None => return Ok(false),
        };

        if name != self.name {
            return Ok(false);
        }

        if parse_prefix_initiated(rest, "@").is_some() {
            // The command looks like a specific command. If we treated it like
            // a general command match, we would interpret other bots' specific
            // commands as general commands.
            return Ok(false);
        }

        self.inner.execute(rest, msg, ctx, bot).await
    }
}

pub struct Specific<C> {
    prefix: String,
    name: String,
    inner: C,
}

impl<C> Specific<C> {
    pub fn new<S: ToString>(name: S, inner: C) -> Self {
        Self {
            prefix: "!".to_string(),
            name: name.to_string(),
            inner,
        }
    }

    pub fn prefix<S: ToString>(mut self, prefix: S) -> Self {
        self.prefix = prefix.to_string();
        self
    }
}

#[async_trait]
impl<B, E, C> Command<B, E> for Specific<C>
where
    B: Send,
    C: Command<B, E> + Send + Sync,
{
    fn description(&self, ctx: &Context) -> Option<String> {
        let inner = self.inner.description(ctx)?;
        let nick = nick::mention(&ctx.joined.session.name);
        Some(format!("{}{} @{nick} - {inner}", self.prefix, self.name))
    }

    async fn execute(
        &self,
        arg: &str,
        msg: &Message,
        ctx: &Context,
        bot: &mut B,
    ) -> Result<bool, E> {
        // TODO Replace with let-else
        let (name, rest) = match parse_prefix_initiated(arg, &self.prefix) {
            Some(parsed) => parsed,
            None => return Ok(false),
        };

        if name != self.name {
            return Ok(false);
        }

        // TODO Replace with let-else
        let (nick, rest) = match parse_prefix_initiated(rest, "@") {
            Some(parsed) => parsed,
            None => return Ok(false),
        };

        if nick::normalize(nick) != nick::normalize(&ctx.joined.session.name) {
            return Ok(false);
        }

        self.inner.execute(rest, msg, ctx, bot).await
    }
}

#[cfg(test)]
mod test {
    use super::parse_prefix_initiated;

    #[test]
    fn test_parse_prefixed() {
        assert_eq!(parse_prefix_initiated("!foo", "!"), Some(("foo", "")));
        assert_eq!(parse_prefix_initiated("    !foo", "!"), Some(("foo", "")));
        assert_eq!(
            parse_prefix_initiated("!foo    ", "!"),
            Some(("foo", "   "))
        );
        assert_eq!(
            parse_prefix_initiated("    !foo    ", "!"),
            Some(("foo", "   "))
        );
        assert_eq!(
            parse_prefix_initiated("!foo @bar", "!"),
            Some(("foo", "@bar"))
        );
        assert_eq!(
            parse_prefix_initiated("!foo    @bar", "!"),
            Some(("foo", "   @bar"))
        );
        assert_eq!(
            parse_prefix_initiated("!foo @bar   ", "!"),
            Some(("foo", "@bar   "))
        );
        assert_eq!(parse_prefix_initiated("! foo @bar", "!"), None);
        assert_eq!(parse_prefix_initiated("!", "!"), None);
        assert_eq!(parse_prefix_initiated("?foo", "!"), None);
    }
}
