//! Euphoria-style `!foo` and `!foo @bar` command wrappers.

use async_trait::async_trait;
use euphoxide::{api::Message, nick};

use super::{Command, Context, Info, Propagate};

// TODO Don't ignore leading whitespace?
// I'm not entirely happy with how commands handle whitespace, and on euphoria,
// prefixing commands with whitespace is traditionally used to not trigger them.

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
    pub prefix: String,
    pub name: String,
    pub inner: C,
}

impl<C> Global<C> {
    pub fn new<S: ToString>(name: S, inner: C) -> Self {
        Self {
            prefix: "!".to_string(),
            name: name.to_string(),
            inner,
        }
    }

    pub fn with_prefix<S: ToString>(mut self, prefix: S) -> Self {
        self.prefix = prefix.to_string();
        self
    }
}

#[async_trait]
impl<E, C> Command<E> for Global<C>
where
    C: Command<E> + Sync,
{
    fn info(&self, ctx: &Context<E>) -> Info {
        self.inner
            .info(ctx)
            .with_prepended_trigger(format!("{}{}", self.prefix, self.name))
    }

    async fn execute(&self, arg: &str, msg: &Message, ctx: &Context<E>) -> Result<Propagate, E> {
        let Some((name, rest)) = parse_prefix_initiated(arg, &self.prefix) else {
            return Ok(Propagate::Yes);
        };

        if name != self.name {
            return Ok(Propagate::Yes);
        }

        self.inner.execute(rest, msg, ctx).await
    }
}

pub struct General<C> {
    pub prefix: String,
    pub name: String,
    pub inner: C,
}

impl<C> General<C> {
    pub fn new<S: ToString>(name: S, inner: C) -> Self {
        Self {
            prefix: "!".to_string(),
            name: name.to_string(),
            inner,
        }
    }

    pub fn with_prefix<S: ToString>(mut self, prefix: S) -> Self {
        self.prefix = prefix.to_string();
        self
    }
}

#[async_trait]
impl<E, C> Command<E> for General<C>
where
    C: Command<E> + Sync,
{
    fn info(&self, ctx: &Context<E>) -> Info {
        self.inner
            .info(ctx)
            .with_prepended_trigger(format!("{}{}", self.prefix, self.name))
    }

    async fn execute(&self, arg: &str, msg: &Message, ctx: &Context<E>) -> Result<Propagate, E> {
        let Some((name, rest)) = parse_prefix_initiated(arg, &self.prefix) else {
            return Ok(Propagate::Yes);
        };

        if name != self.name {
            return Ok(Propagate::Yes);
        }

        if parse_prefix_initiated(rest, "@").is_some() {
            // The command looks like a specific command. If we treated it like
            // a general command match, we would interpret other bots' specific
            // commands as general commands.
            return Ok(Propagate::Yes);
        }

        self.inner.execute(rest, msg, ctx).await
    }
}

pub struct Specific<C> {
    pub prefix: String,
    pub name: String,
    pub inner: C,
}

impl<C> Specific<C> {
    pub fn new<S: ToString>(name: S, inner: C) -> Self {
        Self {
            prefix: "!".to_string(),
            name: name.to_string(),
            inner,
        }
    }

    pub fn with_prefix<S: ToString>(mut self, prefix: S) -> Self {
        self.prefix = prefix.to_string();
        self
    }
}

#[async_trait]
impl<E, C> Command<E> for Specific<C>
where
    C: Command<E> + Sync,
{
    fn info(&self, ctx: &Context<E>) -> Info {
        let nick = nick::mention(&ctx.joined.session.name);
        self.inner
            .info(ctx)
            .with_prepended_trigger(format!("{}{} @{nick}", self.prefix, self.name))
    }

    async fn execute(&self, arg: &str, msg: &Message, ctx: &Context<E>) -> Result<Propagate, E> {
        let Some((name, rest)) = parse_prefix_initiated(arg, &self.prefix) else {
            return Ok(Propagate::Yes);
        };

        if name != self.name {
            return Ok(Propagate::Yes);
        }

        let Some((nick, rest)) = parse_prefix_initiated(rest, "@") else {
            return Ok(Propagate::Yes);
        };

        if nick::normalize(nick) != nick::normalize(&ctx.joined.session.name) {
            return Ok(Propagate::Yes);
        }

        self.inner.execute(rest, msg, ctx).await
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
