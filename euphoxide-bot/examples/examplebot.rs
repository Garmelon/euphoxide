use std::time::Duration;

use async_trait::async_trait;
use euphoxide::api::Message;
use euphoxide_bot::{
    botrulez::{FullHelp, Ping, ShortHelp},
    Command, CommandExt, Commands, Context, Info, Propagate,
};
use euphoxide_client::MultiClient;
use log::error;
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

#[tokio::main]
async fn main() {
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
        .then(Pyramid.general("pyramid"))
        .build();

    let clients = MultiClient::new(event_tx);

    clients
        .client_builder("test")
        .with_username("examplebot")
        .build_and_add()
        .await;

    while let Some(event) = event_rx.recv().await {
        let commands = commands.clone();
        let clients = clients.clone();
        tokio::task::spawn(async move {
            if let Err(err) = commands.handle_event(clients, event).await {
                error!("Oops: {err}")
            }
        });
    }
}
