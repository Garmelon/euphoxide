use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use euphoxide::{
    api::{Data, Message, Nick, ParsedPacket, Send},
    client::{conn::ClientConnHandle, state::State},
};
use euphoxide_bot::{
    bot::Bot,
    command::{
        botrulez::{FullHelp, HasCommandInfos, HasStartTime, Ping, ShortHelp, Uptime},
        Command, CommandExt, Commands, Context, Info, Propagate,
    },
    instance::ServerConfig,
};
use jiff::Timestamp;

struct Pyramid;

#[async_trait]
impl Command<BotState> for Pyramid {
    fn info(&self, _ctx: &Context) -> Info {
        Info::new().with_description("build a pyramid")
    }

    async fn execute(
        &self,
        _arg: &str,
        msg: &Message,
        ctx: &Context,
        _bot: &BotState,
    ) -> euphoxide::Result<Propagate> {
        let mut parent = msg.id;

        for _ in 0..3 {
            let first = ctx.reply(parent, "brick").await?;
            ctx.reply_only(parent, "brick").await?;
            parent = first.await?.0.id;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        ctx.reply_only(parent, "brick").await?;
        Ok(Propagate::No)
    }
}

#[derive(Clone)]
struct BotState {
    start_time: Timestamp,
    commands: Arc<Commands<Self>>,
}

impl HasStartTime for BotState {
    fn start_time(&self) -> Timestamp {
        self.start_time
    }
}

impl HasCommandInfos for BotState {
    fn command_infos(&self, ctx: &Context) -> Vec<Info> {
        self.commands.infos(ctx)
    }
}

async fn run() -> anyhow::Result<()> {
    let commands = Commands::new()
        .then(Ping::default())
        .then(Uptime)
        .then(ShortHelp::new("/me demonstrates how to use euphoxide"))
        .then(FullHelp::new())
        .then(Pyramid.global("pyramid"));

    let commands = Arc::new(commands);

    let state = BotState {
        start_time: Timestamp::now(),
        commands: commands.clone(),
    };

    let mut bot = Bot::new();

    let config = ServerConfig::default()
        .instance("test")
        .with_username("examplebot");

    bot.add_instance(config);

    while let Some(event) = bot.recv().await {
        commands.on_bot_event(event, &state).await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    loop {
        if let Err(err) = run().await {
            println!("Error while running: {err}");
        }
    }
}
