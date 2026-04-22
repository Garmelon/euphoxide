//! Basic command wrappers.

use async_trait::async_trait;

use super::{Command, Context, Info, Propagate};

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

pub struct Described<C> {
    pub inner: C,
    pub description: String,
}

impl<C> Described<C> {
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
    fn info(&self, ctx: &Context<D, E>) -> Info {
        let mut info = self.inner.info(ctx);
        info.description = Some(self.description.clone());
        info
    }

    async fn execute(&self, ctx: &Context<D, E>, arg: &str) -> Result<Propagate, E> {
        self.inner.execute(ctx, arg).await
    }
}

pub struct Prefixed<C> {
    pub prefix: String,
    pub inner: C,
}

impl<C> Prefixed<C> {
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
    fn info(&self, ctx: &Context<D, E>) -> Info {
        let mut info = self.inner.info(ctx);
        info.prepend_trigger(&self.prefix);
        info
    }

    async fn execute(&self, ctx: &Context<D, E>, arg: &str) -> Result<Propagate, E> {
        if let Some(rest) = arg.trim_start().strip_prefix(&self.prefix) {
            self.inner.execute(ctx, rest).await
        } else {
            Ok(Propagate::Yes)
        }
    }
}

// Black type magic, thanks a lot to https://github.com/kpreid and the
// async_fn_traits crate!

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

pub struct FromHandler<F>(pub F);

impl<F> FromHandler<F> {
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
