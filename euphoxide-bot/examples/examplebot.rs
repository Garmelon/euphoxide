use std::time::Duration;

use async_trait::async_trait;
use euphoxide::api::Message;
use euphoxide_bot::{
    bot::Bot,
    command::{
        botrulez::{FullHelp, Ping, ShortHelp},
        Command, CommandExt, Commands, Context, Info, Propagate,
    },
};
use tokio::sync::mpsc;

struct Pyramid;

#[async_trait]
impl Command for Pyramid {
    fn info(&self, _ctx: &Context) -> Info {
        Info::new().with_description("build a pyramid")
    }

    async fn execute(
        &self,
        _arg: &str,
        msg: &Message,
        ctx: &Context,
        _bot: &Bot,
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

async fn run() -> anyhow::Result<()> {
    let (event_tx, mut event_rx) = mpsc::channel(10);

    let commands = Commands::new()
        .then(Ping::default().general("ping").hidden())
        .then(Ping::default().specific("ping").hidden())
        .then(
            ShortHelp::new("/me demonstrates how to use euphoxide")
                .general("help")
                .hidden(),
        )
        .then(
            FullHelp::new()
                .with_after("Created using euphoxide.")
                .specific("help")
                .hidden(),
        )
        .then(Pyramid.general("pyramid"));

    let bot: Bot = Bot::new_simple(commands, event_tx);

    bot.instances
        .add_instance(
            bot.server_config
                .clone()
                .instance("test")
                .with_username("examplebot"),
        )
        .await;

    while let Some(event) = event_rx.recv().await {
        bot.handle_event(event);
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
