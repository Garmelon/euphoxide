use std::time::Duration;

use euphoxide::{
    api::{Data, Message, Nick, Send},
    client::conn::ClientConnHandle,
};
use euphoxide_client::{MultiClient, MultiClientEvent};
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
    let (event_tx, mut event_rx) = mpsc::channel(10);

    // Don't drop the client or it will stop running.
    let clients = MultiClient::new(event_tx);

    clients
        .client_builder("test")
        .with_username("examplebot")
        .build_and_add()
        .await;

    while let Some(event) = event_rx.recv().await {
        if let MultiClientEvent::Packet { conn, packet, .. } = event {
            let data = packet.into_data()?;
            tokio::task::spawn(on_data(conn, data));
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
