//! Similar to the `testbot_manual` example, but using [`Instance`] to connect
//! to the room (and to reconnect).

use euphoxide::api::packet::ParsedPacket;
use euphoxide::api::{Data, Nick, Send};
use euphoxide::bot::botrulez;
use euphoxide::bot::instance::{ConnSnapshot, Event, ServerConfig};
use euphoxide::bot::instances::Instances;
use jiff::Timestamp;
use tokio::sync::mpsc;

const NICK: &str = "TestBot";
const HELP: &str = "I'm an example bot for https://github.com/Garmelon/euphoxide";

async fn on_packet(packet: ParsedPacket, snapshot: ConnSnapshot) -> Result<(), ()> {
    let data = match packet.content {
        Ok(data) => data,
        Err(err) => {
            println!("Error for {}: {err}", packet.r#type);
            return Err(());
        }
    };

    match data {
        Data::HelloEvent(ev) => println!("Connected with id {}", ev.session.id),
        Data::SnapshotEvent(ev) => {
            for session in ev.listing {
                println!("{:?} ({}) is already here", session.name, session.id);
            }

            // Here, a new task is spawned so the main event loop can
            // continue running immediately instead of waiting for a reply
            // from the server.
            //
            // We only need to do this because we want to log the result of
            // the nick command. Otherwise, we could've just called
            // tx.send() synchronously and ignored the returned Future.
            let conn_tx_clone = snapshot.conn_tx.clone();
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
                if let Some(joined) = snapshot.state.joined() {
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
                let _ = snapshot
                    .conn_tx
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
                snapshot.conn_tx.send_only(Send {
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
async fn main() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut instances = Instances::new(ServerConfig::default());

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

        if let Event::Packet(_config, packet, snapshot) = event {
            if on_packet(packet, snapshot).await.is_err() {
                break;
            }
        }
    }
}
