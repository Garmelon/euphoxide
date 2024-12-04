// TODO Add description
// TODO Clean up and unify test bots

use std::sync::Arc;

use async_trait::async_trait;
use clap::Parser;
use euphoxide::api::Message;
use euphoxide::bot::botrulez::{FullHelp, HasDescriptions, HasStartTime, Ping, ShortHelp, Uptime};
use euphoxide::bot::command::{Clap, ClapCommand, Context, General, Global, Hidden, Specific};
use euphoxide::bot::commands::Commands;
use euphoxide::bot::instance::{Event, ServerConfig};
use euphoxide::bot::instances::Instances;
use euphoxide::conn;
use jiff::Timestamp;
use log::error;
use tokio::sync::mpsc;

const HELP: &str = "I'm an example bot for https://github.com/Garmelon/euphoxide";

/// Kill this bot.
#[derive(Parser)]
struct KillArgs;

struct Kill;

#[async_trait]
impl ClapCommand<Bot, conn::Error> for Kill {
    type Args = KillArgs;

    async fn execute(
        &self,
        _args: Self::Args,
        msg: &Message,
        ctx: &Context,
        bot: &mut Bot,
    ) -> Result<bool, conn::Error> {
        bot.stop = true;
        ctx.reply(msg.id, "/me dies").await?;
        Ok(true)
    }
}

/// Do some testing.
#[derive(Parser)]
struct TestArgs {
    /// How much testing to do.
    #[arg(default_value_t = 1)]
    amount: u64,
}

struct Test;

#[async_trait]
impl ClapCommand<Bot, conn::Error> for Test {
    type Args = TestArgs;

    async fn execute(
        &self,
        args: Self::Args,
        msg: &Message,
        ctx: &Context,
        _bot: &mut Bot,
    ) -> Result<bool, conn::Error> {
        let content = if args.amount == 1 {
            format!("/me did {} test", args.amount)
        } else {
            format!("/me did {} tests", args.amount)
        };
        ctx.reply(msg.id, content).await?;
        Ok(true)
    }
}

struct Bot {
    commands: Arc<Commands<Self, conn::Error>>,
    start_time: Timestamp,
    stop: bool,
}

impl HasDescriptions for Bot {
    fn descriptions(&self, ctx: &Context) -> Vec<String> {
        self.commands.descriptions(ctx)
    }
}

impl HasStartTime for Bot {
    fn start_time(&self) -> Timestamp {
        self.start_time
    }
}

#[tokio::main]
async fn main() {
    // https://github.com/snapview/tokio-tungstenite/issues/353#issuecomment-2455247837
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .unwrap();

    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut instances = Instances::new(ServerConfig::default());

    let mut cmds = Commands::new();
    cmds.add(Hidden(General::new("ping", Clap(Ping::default()))));
    cmds.add(Specific::new("ping", Clap(Ping::default())));
    cmds.add(Hidden(General::new("help", Clap(ShortHelp::new(HELP)))));
    cmds.add(Specific::new("help", Clap(FullHelp::new(HELP, ""))));
    cmds.add(Specific::new("uptime", Clap(Uptime)));
    cmds.add(Specific::new("kill", Clap(Kill)));
    cmds.add(Global::new("test", Clap(Test)));
    let cmds = Arc::new(cmds);

    let mut bot = Bot {
        commands: cmds.clone(),
        start_time: Timestamp::now(),
        stop: false,
    };

    for room in ["test", "test2", "testing"] {
        let tx_clone = tx.clone();
        let instance = instances
            .server_config()
            .clone()
            .room(room)
            .username(Some("TestBot"))
            .build(move |e| {
                let _ = tx_clone.send(e);
            });
        instances.add(instance);
    }

    while let Some(event) = rx.recv().await {
        instances.purge();
        if instances.is_empty() {
            break;
        }

        if let Event::Packet(config, packet, snapshot) = event {
            let result = cmds
                .handle_packet(&config, &packet, &snapshot, &mut bot)
                .await;
            if let Err(err) = result {
                error!("{err}");
            }
            if bot.stop {
                break;
            }
        }
    }
}
