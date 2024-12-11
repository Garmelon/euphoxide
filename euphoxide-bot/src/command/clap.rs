//! [`clap`]-based commands.

use async_trait::async_trait;
use clap::{CommandFactory, Parser};
use euphoxide::api::Message;

use crate::bot::Bot;

use super::{Command, Context, Info, Propagate};

#[async_trait]
pub trait ClapCommand<S, E> {
    type Args;

    async fn execute(
        &self,
        args: Self::Args,
        msg: &Message,
        ctx: &Context,
        bot: &Bot<S, E>,
    ) -> Result<Propagate, E>;
}

/// Parse bash-like quoted arguments separated by whitespace.
///
/// Outside of quotes, the backslash either escapes the next character or forms
/// an escape sequence. \n is a newline, \r a carriage return and \t a tab.
/// TODO Escape sequences
///
/// Special characters like the backslash and whitespace can also be quoted
/// using double quotes. Within double quotes, \" escapes a double quote and \\
/// escapes a backslash. Other occurrences of \ have no special meaning.
fn parse_quoted_args(text: &str) -> Result<Vec<String>, &'static str> {
    let mut args = vec![];
    let mut arg = String::new();
    let mut arg_exists = false;

    let mut quoted = false;
    let mut escaped = false;
    for c in text.chars() {
        if quoted {
            match c {
                '\\' if escaped => {
                    arg.push('\\');
                    escaped = false;
                }
                '"' if escaped => {
                    arg.push('"');
                    escaped = false;
                }
                c if escaped => {
                    arg.push('\\');
                    arg.push(c);
                    escaped = false;
                }
                '\\' => escaped = true,
                '"' => quoted = false,
                c => arg.push(c),
            }
        } else {
            match c {
                c if escaped => {
                    arg.push(c);
                    arg_exists = true;
                    escaped = false;
                }
                c if c.is_whitespace() => {
                    if arg_exists {
                        args.push(arg);
                        arg = String::new();
                        arg_exists = false;
                    }
                }
                '\\' => escaped = true,
                '"' => {
                    quoted = true;
                    arg_exists = true;
                }
                c => {
                    arg.push(c);
                    arg_exists = true;
                }
            }
        }
    }

    if quoted {
        return Err("Unclosed trailing quote");
    }
    if escaped {
        return Err("Unfinished trailing escape");
    }

    if arg_exists {
        args.push(arg);
    }

    Ok(args)
}

pub struct Clap<C>(pub C);

#[async_trait]
impl<S, E, C> Command<S, E> for Clap<C>
where
    S: Send + Sync,
    E: From<euphoxide::Error>,
    C: ClapCommand<S, E> + Sync,
    C::Args: Parser + Send,
{
    fn info(&self, _ctx: &Context) -> Info {
        Info {
            description: C::Args::command().get_about().map(|s| s.to_string()),
            ..Info::default()
        }
    }

    async fn execute(
        &self,
        arg: &str,
        msg: &Message,
        ctx: &Context,
        bot: &Bot<S, E>,
    ) -> Result<Propagate, E> {
        let mut args = match parse_quoted_args(arg) {
            Ok(args) => args,
            Err(err) => {
                ctx.reply_only(msg.id, err).await?;
                return Ok(Propagate::No);
            }
        };

        // Hacky, but it should work fine in most cases
        let usage = msg.content.strip_suffix(arg).unwrap_or("<command>").trim();
        args.insert(0, usage.to_string());

        let args = match C::Args::try_parse_from(args) {
            Ok(args) => args,
            Err(err) => {
                ctx.reply_only(msg.id, format!("{}", err.render())).await?;
                return Ok(Propagate::No);
            }
        };

        self.0.execute(args, msg, ctx, bot).await
    }
}

#[cfg(test)]
mod test {
    use super::parse_quoted_args;

    fn assert_quoted(raw: &str, parsed: &[&str]) {
        let parsed = parsed.iter().map(|s| s.to_string()).collect();
        assert_eq!(parse_quoted_args(raw), Ok(parsed))
    }

    #[test]
    fn test_parse_quoted_args() {
        assert_quoted("foo bar baz", &["foo", "bar", "baz"]);
        assert_quoted("    foo     bar     baz    ", &["foo", "bar", "baz"]);
        assert_quoted("foo\\ ba\"r ba\"z", &["foo bar baz"]);
        assert_quoted(
            "It's a nice day, isn't it?",
            &["It's", "a", "nice", "day,", "isn't", "it?"],
        );

        // Trailing whitespace
        assert_quoted("a ", &["a"]);
        assert_quoted("a\\ ", &["a "]);
        assert_quoted("a\\  ", &["a "]);

        // Zero-length arguments
        assert_quoted("a \"\" b \"\"", &["a", "", "b", ""]);
        assert_quoted("a \"\" b \"\" ", &["a", "", "b", ""]);

        // Backslashes in quotes
        assert_quoted("\"a \\b \\\" \\\\\"", &["a \\b \" \\"]);

        // Unclosed quotes and unfinished escapes
        assert!(parse_quoted_args("foo 'bar \"baz").is_err());
        assert!(parse_quoted_args("foo \"bar baz").is_err());
        assert!(parse_quoted_args("foo \"bar 'baz").is_err());
        assert!(parse_quoted_args("foo \\").is_err());
        assert!(parse_quoted_args("foo 'bar\\").is_err());
    }
}
