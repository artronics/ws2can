use std::borrow::Borrow;
use std::net::SocketAddr;
use std::result::Result::Ok;

use futures_util::{
    future, pin_mut, stream::TryStreamExt, FutureExt, SinkExt, StreamExt, TryFutureExt,
};
use log::*;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{self, oneshot};
use tokio_socketcan::{CANFrame, CANSocket, Error};
use tokio_tungstenite::tungstenite::Message;

use can::CanFrame;

type Sender<T = CANFrame> = sync::mpsc::Sender<T>;
type Receiver<T = CANFrame> = sync::mpsc::Receiver<T>;

mod can;

async fn handle_ws_connection(
    raw_stream: TcpStream,
    addr: SocketAddr,
    in_tx: &Sender,
    mut out_rx: Receiver,
) {
    info!("Incoming TCP connection from: {}", addr);

    let stream = tokio_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");
    info!("WebSocket connection established: {}", addr);

    let (mut outgoing, incoming) = stream.split();

    let ws_receiver = incoming.for_each(|msg| async move {
        match msg {
            Ok(msg) => {
                if let Ok(msg) = msg.to_text() {
                    if let Ok(can_frame) = CanFrame::from_json(msg) {
                        info!("WS(in): {}", msg);
                        in_tx.clone().send(can_frame.to_linux_frame()).await;
                    } else {
                        error!("Couldn't parse received can frame json: {}", msg);
                    }
                }
            }
            Err(e) => {
                error!("Error occurred while WS receiving a message: {:?}", e);
            }
        }
    });

    let ws_transmitter = tokio::spawn(async move {
        while let Some(f) = out_rx.recv().await {
            let j = CanFrame::from_linux_frame(f).to_json();
            info!("WS(out): {}", j);
            let msg = Message::Text(j);
            outgoing.send(msg).await;
        }
    });

    pin_mut!(ws_receiver, ws_transmitter);
    future::select(ws_receiver, ws_transmitter).await;
    info!("WS disconnected!");
}

async fn start_ws(addr: &str, in_tx: &Sender, out_rx: Receiver) {
    let listener = TcpListener::bind(addr)
        .await
        .expect(&format!("Can't bind websocket address {}", addr));

    info!("Listening on: {}", addr);

    if let Ok((stream, addr)) = listener.accept().await {
        handle_ws_connection(stream, addr, &in_tx, out_rx).await;
    }
}

async fn start_can(
    can_addr: &str,
    out_tx: &Sender,
    mut in_rx: Receiver,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut can_socket = CANSocket::open(&can_addr)?;
    let (mut outgoing, incoming) = can_socket.split();

    let can_receiver = incoming.for_each(|msg| async move {
        match msg {
            Ok(msg) => {
                info!("CAN(in): {:?}", msg);
                out_tx.clone().send(msg).await;
            }
            Err(e) => {
                error!("Error occurred while CAN receiving a message: {:?}", e);
            }
        }
    });

    let can_transmitter = tokio::spawn(async move {
        while let Some(f) = in_rx.recv().await {
            info!("CAN(out): {:?}", f);
            outgoing.send(f).await;
        }
    });

    tokio::join!(can_transmitter, can_receiver);

    Ok(())
}

///       (in_tx   ,   in_rx )      = sync::mpsc::channel();
///       (out_tx  ,   out_rx)      = sync::mpsc::channel();
///     -> in_tx ----> in_rx ->
///  WS                          CAN
///     <- out_tx <---- out_rx <-
pub async fn run(ws_addr: String, can_addr: String) -> Result<(), Box<dyn std::error::Error>> {
    let (in_tx, mut in_rx) = sync::mpsc::channel(100);
    let (out_tx, mut out_rx) = sync::mpsc::channel(100);

    let ws = start_ws(&ws_addr, &in_tx, out_rx);
    let can = start_can(&can_addr, &out_tx, in_rx);

    tokio::join!(ws, can);

    Ok(())
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::*;

    #[tokio::test]
    async fn it_works() {
        let (tx, mut rx) = mpsc::channel(1);
        tx.clone().send(100).await;

        let s = tokio::spawn(async move {
            for i in 0..10 {
                if let Err(_) = tx.send(i).await {
                    println!("receiver dropped");
                    return;
                }
            }
        });

        let r = tokio::spawn(async move {
            while let Some(i) = rx.recv().await {
                println!("got = {}", i);
            }
        });
        tokio::join!(s, r);
    }
}
