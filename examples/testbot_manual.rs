//! A small bot that doesn't use the `bot` submodule. Meant to show how the main
//! parts of the API fit together.

use std::error::Error;
use std::time::Duration;

use euphoxide::api::packet::ParsedPacket;
use euphoxide::api::{Data, Nick, Send};
use euphoxide::bot::botrulez;
use euphoxide::conn::{Conn, ConnTx, State};
use jiff::Timestamp;

const TIMEOUT: Duration = Duration::from_secs(10);
const DOMAIN: &str = "euphoria.leet.nu";
const ROOM: &str = "test";
const NICK: &str = "TestBot";
const HELP: &str = "I'm an example bot for https://github.com/Garmelon/euphoxide";

async fn on_packet(packet: ParsedPacket, conn_tx: &ConnTx, state: &State) -> Result<(), ()> {
    let data = match packet.content {
        Ok(data) => data,
        Err(err) => {
            println!("Error for {}: {err}", packet.r#type);
            return Err(());
        }
    };

    match data {
        Data::HelloEvent(event) => println!("Connected with id {}", event.session.id),
        Data::SnapshotEvent(event) => {
            for session in event.listing {
                println!("{:?} ({}) is already here", session.name, session.id);
            }

            // Here, a new task is spawned so the main event loop can
            // continue running immediately instead of waiting for a reply
            // from the server.
            //
            // We only need to do this because we want to log the result of
            // the nick command. Otherwise, we could've just called
            // tx.send() synchronously and ignored the returned Future.
            let conn_tx_clone = conn_tx.clone();
            tokio::spawn(async move {
                // Awaiting the future returned by the send command lets you
                // (type-safely) access the server's reply.
                let reply = conn_tx_clone
                    .send(Nick {
                        name: NICK.to_string(),
                    })
                    .await;
                match reply {
                    Ok(reply) => println!("Set nick to {:?}", reply.to),
                    Err(err) => println!("Failed to set nick: {err}"),
                };
            });
        }
        Data::BounceEvent(_) => {
            println!("Received bounce event, stopping");
            return Err(());
        }
        Data::DisconnectEvent(_) => {
            println!("Received disconnect event, stopping");
            return Err(());
        }
        Data::JoinEvent(event) => println!("{:?} ({}) joined", event.0.name, event.0.id),
        Data::PartEvent(event) => println!("{:?} ({}) left", event.0.name, event.0.id),
        Data::NickEvent(event) => println!(
            "{:?} ({}) is now known as {:?}",
            event.from, event.id, event.to
        ),
        Data::SendEvent(event) => {
            println!("Message {} was just sent", event.0.id.0);

            let content = event.0.content.trim();
            let mut reply = None;

            if content == "!ping" || content == format!("!ping @{NICK}") {
                reply = Some("Pong!".to_string());
            } else if content == format!("!help @{NICK}") {
                reply = Some(HELP.to_string());
            } else if content == format!("!uptime @{NICK}") {
                if let Some(joined) = state.joined() {
                    let delta = Timestamp::now() - joined.since;
                    reply = Some(format!(
                        "/me has been up for {}",
                        botrulez::format_duration(delta)
                    ));
                }
            } else if content == "!test" {
                reply = Some("Test successful!".to_string());
            } else if content == format!("!kill @{NICK}") {
                println!(
                    "I was killed by {:?} ({})",
                    event.0.sender.name, event.0.sender.id
                );
                // Awaiting the server reply in the main loop to ensure the
                // message is sent before we exit the loop. Otherwise, there
                // would be a race between sending the message and closing
                // the connection as the send function can return before the
                // message has actually been sent.
                let _ = conn_tx
                    .send(Send {
                        content: "/me dies".to_string(),
                        parent: Some(event.0.id),
                    })
                    .await;
                return Err(());
            }

            if let Some(reply) = reply {
                // If you are not interested in the result, you can just
                // throw away the future returned by the send function.
                println!("Sending reply...");
                conn_tx.send_only(Send {
                    content: reply,
                    parent: Some(event.0.id),
                });
                println!("Reply sent!");
            }
        }
        _ => {}
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // https://github.com/snapview/tokio-tungstenite/issues/353#issuecomment-2455247837
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .unwrap();

    let (mut conn, _) = Conn::connect(DOMAIN, ROOM, false, None, TIMEOUT).await?;

    while let Ok(packet) = conn.recv().await {
        if on_packet(packet, conn.tx(), conn.state()).await.is_err() {
            break;
        }
    }
    Ok(())
}
