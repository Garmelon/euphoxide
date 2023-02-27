use async_trait::async_trait;
use clap::Parser;
use time::macros::format_description;
use time::{Duration, OffsetDateTime, UtcOffset};

use crate::api::Message;
use crate::bot::command::{ClapCommand, Command, Context};
use crate::conn;

pub fn format_time(t: OffsetDateTime) -> String {
    let t = t.to_offset(UtcOffset::UTC);
    let format = format_description!("[year]-[month]-[day] [hour]:[minute]:[second] UTC");
    t.format(format).unwrap()
}

pub fn format_duration(d: Duration) -> String {
    let d_abs = d.abs();
    let days = d_abs.whole_days();
    let hours = d_abs.whole_hours() % 24;
    let mins = d_abs.whole_minutes() % 60;
    let secs = d_abs.whole_seconds() % 60;

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
        format!("in {segments}")
    } else {
        format!("{segments} ago")
    }
}

pub struct Uptime;

pub trait HasStartTime {
    fn start_time(&self) -> OffsetDateTime;
}

impl Uptime {
    fn formulate_reply<B: HasStartTime>(&self, ctx: &Context, bot: &B, connected: bool) -> String {
        let start = bot.start_time();
        let now = OffsetDateTime::now_utc();

        let mut reply = format!(
            "/me has been up since {} ({})",
            format_time(start),
            format_duration(start - now),
        );

        if connected {
            let since = ctx.joined.since;
            reply.push_str(&format!(
                ", connected since {} ({})",
                format_time(since),
                format_duration(since - now),
            ));
        }

        reply
    }
}

#[async_trait]
impl<B, E> Command<B, E> for Uptime
where
    B: HasStartTime + Send,
    E: From<conn::Error>,
{
    async fn execute(
        &self,
        arg: &str,
        msg: &Message,
        ctx: &Context,
        bot: &mut B,
    ) -> Result<bool, E> {
        if arg.trim().is_empty() {
            let reply = self.formulate_reply(ctx, bot, false);
            ctx.reply(msg.id, reply).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// Show how long the bot has been online.
#[derive(Parser)]
pub struct Args {
    /// Show how long the bot has been connected without interruption.
    #[arg(long, short)]
    connected: bool,
}

#[async_trait]
impl<B, E> ClapCommand<B, E> for Uptime
where
    B: HasStartTime + Send,
    E: From<conn::Error>,
{
    type Args = Args;

    async fn execute(
        &self,
        args: Self::Args,
        msg: &Message,
        ctx: &Context,
        bot: &mut B,
    ) -> Result<bool, E> {
        let reply = self.formulate_reply(ctx, bot, args.connected);
        ctx.reply(msg.id, reply).await?;
        Ok(true)
    }
}
