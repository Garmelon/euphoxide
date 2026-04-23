//! Basic command wrappers.

use async_trait::async_trait;

use super::{Command, CommandHelp, Context, Propagate};

/// Reply with a single message.
pub struct Reply(
    /// The message to reply with.
    pub String,
);

impl Reply {
    /// Create a new [`Reply`].
    pub fn new(content: impl ToString) -> Self {
        Self(content.to_string())
    }
}

#[async_trait]
impl<D, E> Command<D, E> for Reply
where
    D: Send + Sync,
    E: From<euphoxide::Error>,
{
    async fn execute(&self, ctx: &Context<D, E>, _arg: &str) -> Result<Propagate, E> {
        ctx.reply_only(&self.0).await?;
        Ok(Propagate::No)
    }
}

/// Execute a [`Command`] only if the argument is empty.
pub struct Exact<C>(pub C);

#[async_trait]
impl<D, E, C> Command<D, E> for Exact<C>
where
    D: Send + Sync,
    C: Command<D, E> + Sync,
{
    async fn execute(&self, ctx: &Context<D, E>, arg: &str) -> Result<Propagate, E> {
        if !arg.trim().is_empty() {
            return Ok(Propagate::Yes);
        }

        self.0.execute(ctx, arg).await
    }
}

/// Wrap a [`Command`], hiding its [`Command::help`].
pub struct Hidden<C>(pub C);

#[async_trait]
impl<D, E, C> Command<D, E> for Hidden<C>
where
    D: Send + Sync,
    C: Command<D, E> + Sync,
{
    async fn execute(&self, ctx: &Context<D, E>, arg: &str) -> Result<Propagate, E> {
        self.0.execute(ctx, arg).await
    }
}

/// Wrap a [`Command`], overwriting its [`CommandHelp::description`].
pub struct Described<C> {
    /// The wrapped command.
    pub inner: C,
    /// The new description.
    pub description: String,
}

impl<C> Described<C> {
    /// Create a [`Described`] wrapping a command.
    pub fn new(inner: C, description: impl ToString) -> Self {
        Self {
            inner,
            description: description.to_string(),
        }
    }
}

#[async_trait]
impl<D, E, C> Command<D, E> for Described<C>
where
    D: Send + Sync,
    C: Command<D, E> + Sync,
{
    fn help(&self, ctx: &Context<D, E>) -> CommandHelp {
        let mut info = self.inner.help(ctx);
        info.description = Some(self.description.clone());
        info
    }

    async fn execute(&self, ctx: &Context<D, E>, arg: &str) -> Result<Propagate, E> {
        self.inner.execute(ctx, arg).await
    }
}

/// Wrap a [`Command`], requiring a prefix before triggering.
///
/// This wrapper is similar to [`crate::Global`], but it doesn't require a space
/// between the prefix and the arguments.
pub struct Prefixed<C> {
    /// The prefix that is stripped from the command arguments.
    pub prefix: String,
    /// The wrapped command.
    pub inner: C,
}

impl<C> Prefixed<C> {
    /// Create a [`Prefixed`] wrapping a command.
    pub fn new<S: ToString>(prefix: S, inner: C) -> Self {
        Self {
            prefix: prefix.to_string(),
            inner,
        }
    }
}

#[async_trait]
impl<D, E, C> Command<D, E> for Prefixed<C>
where
    D: Send + Sync,
    C: Command<D, E> + Sync,
{
    fn help(&self, ctx: &Context<D, E>) -> CommandHelp {
        let mut info = self.inner.help(ctx);
        info.prepend_trigger(&self.prefix);
        info
    }

    async fn execute(&self, ctx: &Context<D, E>, arg: &str) -> Result<Propagate, E> {
        if let Some(rest) = arg.strip_prefix(&self.prefix) {
            self.inner.execute(ctx, rest).await
        } else {
            Ok(Propagate::Yes)
        }
    }
}

// Black type magic, thanks a lot to https://github.com/kpreid and the
// async_fn_traits crate!

#[expect(missing_docs)]
pub trait HandlerFn<'c, 'm, D, E>: Fn(&'c Context<D, E>, &'m str) -> Self::Future
where
    D: 'c,
    E: 'c,
{
    type Future: Future<Output = Result<Propagate, E>> + Send;
}

impl<'c, 'm, D, E, F, Fut> HandlerFn<'c, 'm, D, E> for F
where
    D: 'c,
    E: 'c,
    F: Fn(&'c Context<D, E>, &'m str) -> Fut + ?Sized,
    Fut: Future<Output = Result<Propagate, E>> + Send,
{
    type Future = Fut;
}

/// Convert a handler function into a [`Command`].
///
/// ```
/// use euphoxide_command::{Context, Propagate, basic::FromHandler};
///
/// async fn handler(ctx: &Context, arg: &str) -> euphoxide::Result<Propagate> {
///   todo!()
/// }
///
/// let cmd = FromHandler::new(handler);
/// ```
pub struct FromHandler<F>(pub F);

impl<F> FromHandler<F> {
    /// Wrap a handler function.
    ///
    /// See [`FromHandler`] for more details.
    pub fn new(f: F) -> Self {
        Self(f)
    }
}

#[async_trait]
impl<D, E, F> Command<D, E> for FromHandler<F>
where
    D: Send + Sync,
    F: for<'c, 'm> HandlerFn<'c, 'm, D, E> + Sync,
{
    async fn execute(&self, ctx: &Context<D, E>, arg: &str) -> Result<Propagate, E> {
        (self.0)(ctx, arg).await
    }
}
