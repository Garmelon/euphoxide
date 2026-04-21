//! Basic command wrappers.

use async_trait::async_trait;

use super::{Command, Context, Info, Propagate};

/// Rewrite or hide command info.
pub struct Described<C> {
    pub inner: C,
    pub trigger: Option<Option<String>>,
    pub description: Option<Option<String>>,
}

impl<C> Described<C> {
    pub fn new(inner: C) -> Self {
        Self {
            inner,
            trigger: None,
            description: None,
        }
    }

    pub fn hidden(inner: C) -> Self {
        Self::new(inner)
            .with_trigger_hidden()
            .with_description_hidden()
    }

    pub fn with_trigger(mut self, trigger: impl ToString) -> Self {
        self.trigger = Some(Some(trigger.to_string()));
        self
    }

    pub fn with_trigger_hidden(mut self) -> Self {
        self.trigger = Some(None);
        self
    }

    pub fn with_description(mut self, description: impl ToString) -> Self {
        self.description = Some(Some(description.to_string()));
        self
    }

    pub fn with_description_hidden(mut self) -> Self {
        self.description = Some(None);
        self
    }
}

#[async_trait]
impl<D, E, C> Command<D, E> for Described<C>
where
    D: Send + Sync,
    C: Command<D, E> + Sync,
{
    fn info(&self, ctx: &Context<D, E>) -> Info {
        let info = self.inner.info(ctx);
        Info {
            trigger: self.trigger.clone().unwrap_or(info.trigger),
            description: self.description.clone().unwrap_or(info.description),
        }
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
        self.inner.info(ctx).with_prepended_trigger(&self.prefix)
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
