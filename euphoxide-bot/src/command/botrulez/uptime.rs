use async_trait::async_trait;
#[cfg(feature = "clap")]
use clap::Parser;
use euphoxide::api::Message;
use jiff::{Span, Timestamp, Unit};

#[cfg(feature = "clap")]
use crate::command::clap::ClapCommand;
use crate::{
    bot::Bot,
    command::{Command, Context, Propagate},
};

pub fn format_time(t: Timestamp) -> String {
    t.strftime("%Y-%m-%d %H:%M:%S UTC").to_string()
}

pub fn format_relative_time(d: Span) -> String {
    if d.is_positive() {
        format!("in {}", format_duration(d.abs()))
    } else {
        format!("{} ago", format_duration(d.abs()))
    }
}

pub fn format_duration(d: Span) -> String {
    let total = d.abs().total(Unit::Second).unwrap() as i64;
    let secs = total % 60;
    let mins = (total / 60) % 60;
    let hours = (total / 60 / 60) % 24;
    let days = total / 60 / 60 / 24;

    let mut segments = vec![];
    if days > 0 {
        segments.push(format!("{days}d"));
    }
    if hours > 0 {
        segments.push(format!("{hours}h"));
    }
    if mins > 0 {
        segments.push(format!("{mins}m"));
    }
    if secs > 0 {
        segments.push(format!("{secs}s"));
    }
    if segments.is_empty() {
        segments.push("0s".to_string());
    }

    let segments = segments.join(" ");
    if d.is_positive() {
        segments
    } else {
        format!("-{segments}")
    }
}

pub struct Uptime;

pub trait HasStartTime {
    fn start_time(&self) -> Timestamp;
}

impl Uptime {
    fn formulate_reply<S, E>(&self, ctx: &Context, bot: &Bot<S, E>, connected: bool) -> String {
        let start = bot.start_time;
        let now = Timestamp::now();

        let mut reply = format!(
            "/me has been up since {} ({})",
            format_time(start),
            format_relative_time(start - now),
        );

        if connected {
            let since = ctx.joined.since;
            reply.push_str(&format!(
                ", connected since {} ({})",
                format_time(since),
                format_relative_time(since - now),
            ));
        }

        reply
    }
}

#[async_trait]
impl<S, E> Command<S, E> for Uptime
where
    S: Send + Sync,
    E: From<euphoxide::Error>,
{
    async fn execute(
        &self,
        arg: &str,
        msg: &Message,
        ctx: &Context,
        bot: &Bot<S, E>,
    ) -> Result<Propagate, E> {
        if arg.trim().is_empty() {
            let reply = self.formulate_reply(ctx, bot, false);
            ctx.reply_only(msg.id, reply).await?;
            Ok(Propagate::No)
        } else {
            Ok(Propagate::Yes)
        }
    }
}

/// Show how long the bot has been online.
#[cfg(feature = "clap")]
#[derive(Parser)]
pub struct UptimeArgs {
    /// Show how long the bot has been connected without interruption.
    #[arg(long, short)]
    pub connected: bool,
}

#[cfg(feature = "clap")]
#[async_trait]
impl<S, E> ClapCommand<S, E> for Uptime
where
    S: Send + Sync,
    E: From<euphoxide::Error>,
{
    type Args = UptimeArgs;

    async fn execute(
        &self,
        args: Self::Args,
        msg: &Message,
        ctx: &Context,
        bot: &Bot<S, E>,
    ) -> Result<Propagate, E> {
        let reply = self.formulate_reply(ctx, bot, args.connected);
        ctx.reply_only(msg.id, reply).await?;
        Ok(Propagate::No)
    }
}
