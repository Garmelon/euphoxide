use std::error::Error;
use std::time::{Duration, Instant};

use euphoxide::api::{Data, Nick, Send};

const URI: &str = "wss://euphoria.io/room/test/ws";
const NICK: &str = "TestBot";
const HELP: &str = "I'm an example bot for https://github.com/Garmelon/euphoxide";

fn format_delta(delta: Duration) -> String {
    const MINUTE: u64 = 60;
    const HOUR: u64 = MINUTE * 60;
    const DAY: u64 = HOUR * 24;

    let mut seconds = delta.as_secs();
    let mut parts = vec![];

    let days = seconds / DAY;
    if days > 0 {
        parts.push(format!("{days}d"));
        seconds -= days * DAY;
    }

    let hours = seconds / HOUR;
    if hours > 0 {
        parts.push(format!("{hours}h"));
        seconds -= hours * HOUR;
    }

    let mins = seconds / MINUTE;
    if mins > 0 {
        parts.push(format!("{mins}m"));
        seconds -= mins * MINUTE;
    }

    if parts.is_empty() || seconds > 0 {
        parts.push(format!("{seconds}s"));
    }

    parts.join(" ")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let start = Instant::now();

    let (ws, _) = tokio_tungstenite::connect_async(URI).await?;
    let (tx, mut rx) = euphoxide::conn::wrap(ws, Duration::from_secs(30));
    while let Some(packet) = rx.recv().await {
        let data = match packet.content {
            Ok(data) => data,
            Err(err) => {
                println!("Error for {}: {err}", packet.r#type);
                continue;
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
                let tx_clone = tx.clone();
                tokio::spawn(async move {
                    // Awaiting the future returned by the send command lets you
                    // (type-safely) access the server's reply.
                    let reply = tx_clone
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
                break;
            }
            Data::DisconnectEvent(_) => {
                println!("Received disconnect event, stopping");
                break;
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
                    let delta = Instant::now().duration_since(start);
                    reply = Some(format!("/me has been up for {}", format_delta(delta)));
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
                    let _ = tx
                        .send(Send {
                            content: "/me dies".to_string(),
                            parent: Some(event.0.id),
                        })
                        .await;
                    break;
                }

                if let Some(reply) = reply {
                    // If you are not interested in the result, you can just
                    // throw away the future returned by the send function.
                    println!("Sending reply...");
                    let _ = tx.send(Send {
                        content: reply,
                        parent: Some(event.0.id),
                    });
                    println!("Reply sent!");
                }
            }
            _ => {}
        }
    }
    Ok(())
}
