//! [`clap`]-based commands.

use std::marker::PhantomData;

use async_trait::async_trait;
use clap::{CommandFactory, Parser};

use super::{Command, CommandHelp, Context, Propagate};

/// A [`Command`] whose arguments are parsed by [`clap`].
#[async_trait]
pub trait ClapCommand<D, E> {
    /// The command arguments that will be parsed by clap.
    type Args;

    /// Execute the command with the parsed arguments.
    async fn execute(&self, ctx: &Context<D, E>, args: Self::Args) -> Result<Propagate, E>;
}

#[async_trait]
impl<D, E, C> ClapCommand<D, E> for &C
where
    D: Send + Sync,
    C: ClapCommand<D, E> + Sync,
    C::Args: Send,
{
    type Args = C::Args;

    async fn execute(&self, ctx: &Context<D, E>, args: Self::Args) -> Result<Propagate, E> {
        (*self).execute(ctx, args).await
    }
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

/// Convert a [`ClapCommand`] into a [`Command`].
pub struct Clap<C>(pub C);

#[async_trait]
impl<D, E, C> Command<D, E> for Clap<C>
where
    D: Send + Sync,
    E: From<euphoxide::Error>,
    C: ClapCommand<D, E> + Sync,
    C::Args: Parser + Send,
{
    fn help(&self, _ctx: &Context<D, E>) -> CommandHelp {
        CommandHelp {
            description: C::Args::command().get_about().map(|s| s.to_string()),
            ..CommandHelp::default()
        }
    }

    async fn execute(&self, ctx: &Context<D, E>, arg: &str) -> Result<Propagate, E> {
        let mut args = match parse_quoted_args(arg) {
            Ok(args) => args,
            Err(err) => {
                ctx.reply_only(err)?;
                return Ok(Propagate::No);
            }
        };

        // Hacky, but it should work fine in most cases
        let usage = ctx
            .msg
            .content
            .strip_suffix(arg)
            .unwrap_or("<command>")
            .trim();
        args.insert(0, usage.to_string());

        let args = match C::Args::try_parse_from(args) {
            Ok(args) => args,
            Err(err) => {
                ctx.reply_only(format!("{}", err.render()))?;
                return Ok(Propagate::No);
            }
        };

        self.0.execute(ctx, args).await
    }
}

#[allow(missing_docs)]
pub trait ClapHandlerFn<'c, A, D, E>: Fn(&'c Context<D, E>, A) -> Self::Future
where
    D: 'c,
    E: 'c,
{
    type Future: Future<Output = Result<Propagate, E>> + Send;
}

impl<'c, A, D, E, F, Fut> ClapHandlerFn<'c, A, D, E> for F
where
    D: 'c,
    E: 'c,
    F: Fn(&'c Context<D, E>, A) -> Fut + ?Sized,
    Fut: Future<Output = Result<Propagate, E>> + Send,
{
    type Future = Fut;
}

/// Convert a handler function into a [`Command`].
///
/// ```
/// use euphoxide_command::{Context, Propagate, clap::FromClapHandler};
///
/// #[derive(clap::Parser)]
/// struct Args {}
///
/// async fn handler(ctx: &Context, args: Args) -> euphoxide::Result<Propagate> {
///   todo!()
/// }
///
/// let cmd = FromClapHandler::new(handler);
/// ```
pub struct FromClapHandler<A, F> {
    _a: PhantomData<A>,
    /// The async handler function.
    pub handler: F,
}

impl<A, F> FromClapHandler<A, F> {
    // Artificially constrained so we don't accidentally choose an incorrect A.
    // Relying on type inference of A can result in unknown type errors even
    // though we know what A should be based on F.

    /// Wrap a handler function.
    ///
    /// See [`FromClapHandler`] for more details.
    pub fn new<'c, D, E, Fut>(handler: F) -> Self
    where
        F: Fn(&'c Context<D, E>, A) -> Fut,
        D: 'c,
        E: 'c,
    {
        Self {
            _a: PhantomData,
            handler,
        }
    }
}

#[async_trait]
impl<A, D, E, F> ClapCommand<D, E> for FromClapHandler<A, F>
where
    A: Send + Sync + 'static,
    D: Send + Sync,
    F: for<'c> ClapHandlerFn<'c, A, D, E> + Sync,
{
    type Args = A;

    async fn execute(&self, ctx: &Context<D, E>, args: Self::Args) -> Result<Propagate, E> {
        (self.handler)(ctx, args).await
    }
}

/// This implementation is provided for convenience so you don't have to always
/// wrap `FromClapHandler` with a `Clap`.
#[async_trait]
impl<A, D, E, F> Command<D, E> for FromClapHandler<A, F>
where
    // These constraints are an unholy amalgam
    // of "impl ClapCommand for FromClapHandler"
    // and "impl Command for Clap".
    A: Parser + Send + Sync + 'static,
    D: Send + Sync,
    E: From<euphoxide::Error>,
    F: for<'c> ClapHandlerFn<'c, A, D, E> + Sync,
{
    fn help(&self, ctx: &Context<D, E>) -> CommandHelp {
        Clap(self).help(ctx)
    }

    async fn execute(&self, ctx: &Context<D, E>, arg: &str) -> Result<Propagate, E> {
        Clap(self).execute(ctx, arg).await
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
