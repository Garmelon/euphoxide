use std::time::Duration;

use euphoxide::api::Message;
use euphoxide_bot::{
    basic::FromHandler,
    botrulez::{FullHelp, Ping, ShortHelp},
    clap::FromClapHandler,
    CommandExt, Commands, Context, Propagate,
};
use euphoxide_client::MultiClient;
use log::error;
use tokio::sync::mpsc;

async fn pyramid(_arg: &str, msg: &Message, ctx: &Context) -> euphoxide::Result<Propagate> {
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

#[derive(clap::Parser)]
struct AddArgs {
    lhs: i64,
    rhs: i64,
}

async fn add(args: AddArgs, msg: &Message, ctx: &Context) -> euphoxide::Result<Propagate> {
    let result = args.lhs + args.rhs;

    ctx.reply_only(msg.id, format!("{} + {} = {result}", args.lhs, args.rhs))
        .await?;

    Ok(Propagate::No)
}

#[tokio::main]
async fn main() {
    let (event_tx, mut event_rx) = mpsc::channel(10);

    let mut commands = Commands::new();

    Ping::default()
        .general("ping")
        .hidden()
        .add_to(&mut commands);

    Ping::default()
        .specific("ping")
        .hidden()
        .add_to(&mut commands);

    ShortHelp::new("/me demonstrates how to use euphoxide")
        .general("help")
        .hidden()
        .add_to(&mut commands);

    FullHelp::new()
        .with_after("Created using euphoxide.")
        .specific("help")
        .hidden()
        .add_to(&mut commands);

    FromHandler::new(pyramid)
        .described()
        .with_description("build a pyramid")
        .general("pyramid")
        .add_to(&mut commands);

    FromClapHandler::new(add)
        .clap()
        .described()
        .with_description("add two numbers")
        .general("add")
        .add_to(&mut commands);

    let commands = commands.build();
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
