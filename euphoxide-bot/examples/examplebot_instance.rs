use std::time::Duration;

use euphoxide::{
    api::{Data, Message, Nick, Send},
    client::conn::ClientConnHandle,
};
use euphoxide_bot::{Instance, InstanceEvent, ServerConfig};
use tokio::sync::mpsc;

async fn set_nick(conn: &ClientConnHandle) -> anyhow::Result<()> {
    conn.send_only(Nick {
        name: "examplebot".to_string(),
    })
    .await?;

    Ok(())
}

async fn send_pong(conn: &ClientConnHandle, msg: Message) -> anyhow::Result<()> {
    conn.send_only(Send {
        content: "Pong!".to_string(),
        parent: Some(msg.id),
    })
    .await?;

    Ok(())
}

async fn send_pyramid(conn: &ClientConnHandle, msg: Message) -> anyhow::Result<()> {
    let mut parent = msg.id;

    for _ in 0..3 {
        let first = conn
            .send(Send {
                content: "brick".to_string(),
                parent: Some(parent),
            })
            .await?;

        conn.send_only(Send {
            content: "brick".to_string(),
            parent: Some(parent),
        })
        .await?;

        parent = first.await?.0.id;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    conn.send_only(Send {
        content: "brick".to_string(),
        parent: Some(parent),
    })
    .await?;

    Ok(())
}

async fn on_data(conn: ClientConnHandle, data: Data) {
    let result = match data {
        Data::SnapshotEvent(_) => set_nick(&conn).await,
        Data::SendEvent(event) if event.0.content == "!ping" => send_pong(&conn, event.0).await,
        Data::SendEvent(event) if event.0.content == "!pyramid" => {
            send_pyramid(&conn, event.0).await
        }
        _ => Ok(()),
    };

    if let Err(err) = result {
        println!("Error while responding: {err}");
    }
}

async fn run() -> anyhow::Result<()> {
    let config = ServerConfig::default()
        .instance("test")
        .with_username("examplebot");

    let (event_tx, mut event_rx) = mpsc::channel(10);
    let _instance = Instance::new((), config, event_tx); // Don't drop or instance stops

    while let Some(event) = event_rx.recv().await {
        if let InstanceEvent::Packet { conn, packet, .. } = event {
            let data = packet.into_data()?;
            on_data(conn, data).await;
        }
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
