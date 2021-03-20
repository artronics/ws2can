use futures_util::{future, pin_mut, stream::TryStreamExt, FutureExt, StreamExt, TryFutureExt};
use log::*;
use std::io;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_socketcan::{CANFrame, CANSocket};

use crate::can::CanFrame;

pub async fn run(ws_addr: &str, can_addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(ws_addr).await?;
    info!("Listening on: {}", ws_addr);

    let (ws_stream, ws_addr) = listener.accept().await?;
    info!("Incoming TCP connection from: {}", ws_addr);

    let ws_stream = tokio_tungstenite::accept_async(ws_stream).await?;
    info!("WebSocket connection established: {}", ws_addr);

    // let can_socket = CANSocket::open(can_addr)?;
    let (can_tx, can_rx) = mpsc::channel(10);

    let (_outgoing, ws_incoming) = ws_stream.split();

    ws_incoming
        .for_each(|msg| async move {
            // info!("Received WS message: {}", msg.to_text().unwrap());
            match msg {
                Ok(msg) => {
                    if let Ok(msg) = msg.to_text() {
                        trace!("Received WS message: {}", msg);
                        if let Ok(can_frame) = CanFrame::from_json(msg) {
                            // info!("get frame {:?}", &can_frame);
                            can_tx.clone().send(can_frame).await;
                        } else {
                            error!("Couldn't parse received can frame json: {}", msg);
                        }
                    } else {
                        error!("Couldn't get message from websocket");
                    }
                }
                Err(e) => {
                    error!("Error occurred while WS receiving a message: {:?}", e);
                }
            }
        })
        .await;
    info!("lis");
    Ok(())
}
