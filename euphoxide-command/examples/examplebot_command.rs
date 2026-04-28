use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use euphoxide_client::Clients;
use euphoxide_command::{
    Command, CommandExt, Commands, Context, Propagate,
    basic::FromHandler,
    botrulez::{FullHelp, Ping, ShortHelp},
    clap::FromClapHandler,
};
use log::error;
use tokio::sync::mpsc;

struct AppData {
    counter: Mutex<usize>,
}

type AppContext = Context<AppData>;

struct Increment;

#[async_trait]
impl Command<AppData> for Increment {
    async fn execute(&self, ctx: &AppContext, _arg: &str) -> euphoxide::Result<Propagate> {
        let count = {
            let mut guard = ctx.data().counter.lock().unwrap();
            *guard += 1;
            *guard
        };

        ctx.reply_only(format!("Counter incremented to {count}"))?;
        Ok(Propagate::No)
    }
}

async fn pyramid(ctx: &AppContext, _arg: &str) -> euphoxide::Result<Propagate> {
    let mut parent = ctx.msg.id;

    for _ in 0..3 {
        let first = ctx.send(Some(parent), "brick")?;
        ctx.send_only(Some(parent), "brick")?;
        parent = first.await?.0.id;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    ctx.send_only(Some(parent), "brick")?;
    Ok(Propagate::No)
}

#[derive(clap::Parser)]
struct AddArgs {
    lhs: i64,
    rhs: i64,
}

async fn add(ctx: &AppContext, args: AddArgs) -> euphoxide::Result<Propagate> {
    let result = args.lhs + args.rhs;

    ctx.reply_only(format!("{} + {} = {result}", args.lhs, args.rhs))?;

    Ok(Propagate::No)
}

#[tokio::main]
async fn main() {
    let (event_tx, mut event_rx) = mpsc::channel(10);

    let mut commands = Commands::new(AppData {
        counter: Mutex::new(0),
    });

    Ping::default()
        .clap()
        .general("ping")
        .hidden()
        .add_to(&mut commands);

    Ping::default()
        .clap()
        .specific("ping")
        .hidden()
        .add_to(&mut commands);

    ShortHelp::new("/me demonstrates how to use euphoxide")
        .clap()
        .general("help")
        .hidden()
        .add_to(&mut commands);

    FullHelp::new()
        .with_after("Created using euphoxide.")
        .clap()
        .specific("help")
        .hidden()
        .add_to(&mut commands);

    Increment
        .general("increment")
        .described("increment a counter")
        .add_to(&mut commands);

    FromHandler::new(pyramid)
        .general("pyramid")
        .described("build a pyramid")
        .add_to(&mut commands);

    FromClapHandler::new(add)
        .general("add")
        .described("add two numbers")
        .add_to(&mut commands);

    let commands = Arc::new(commands);
    let clients = Clients::new(event_tx);

    clients
        .client_builder("test")
        .with_username("examplebot")
        .build_and_add()
        .await;

    while let Some((client, event)) = event_rx.recv().await {
        let commands = commands.clone();
        let clients = clients.clone();
        tokio::task::spawn(async move {
            if let Err(err) = commands.handle_event(clients, client, event).await {
                error!("Oops: {err}")
            }
        });
    }
}
