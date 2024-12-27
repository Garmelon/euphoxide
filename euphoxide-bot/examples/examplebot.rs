use std::time::Duration;

use async_trait::async_trait;
use euphoxide::api::Message;
use euphoxide_bot::{
    bot::Bot,
    command::{
        bang::{General, Specific},
        basic::Described,
        botrulez::{FullHelp, Ping, ShortHelp},
        Command, Commands, Context, Info, Propagate,
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
        .then(Described::hidden(General::new("ping", Ping::default())))
        .then(Described::hidden(Specific::new("ping", Ping::default())))
        .then(Described::hidden(General::new(
            "help",
            ShortHelp::new("/me demonstrates how to use euphoxide"),
        )))
        .then(Described::hidden(Specific::new(
            "help",
            FullHelp::new().with_after("Created using euphoxide."),
        )))
        .then(General::new("pyramid", Pyramid));

    let bot: Bot = Bot::new_simple(commands, event_tx);

    bot.clients
        .client_builder("test")
        .with_username("examplebot")
        .build_and_add()
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
