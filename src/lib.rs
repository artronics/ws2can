use std::borrow::Borrow;
use std::net::SocketAddr;
use std::result::Result::Ok;

use futures_util::{future, pin_mut, stream::TryStreamExt, FutureExt, StreamExt, TryFutureExt};
use log::*;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{self, oneshot};
use tokio_socketcan::{CANFrame, CANSocket, Error};

use can::CanFrame;

type Sender<T = CANFrame> = sync::mpsc::Sender<T>;
type Receiver<T = CANFrame> = sync::mpsc::Receiver<T>;

mod can;

fn a_frame() -> CanFrame {
    CanFrame::new(0x123, [3; 8], 5, false, false)
}

async fn handle_ws_connection(raw_stream: TcpStream, addr: SocketAddr, in_tx: &Sender) {
    info!("Incoming TCP connection from: {}", addr);

    let stream = tokio_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");
    info!("WebSocket connection established: {}", addr);

    let (outgoing, incoming) = stream.split();

    incoming
        .for_each(|msg| async move {
            match msg {
                Ok(msg) => {
                    if let Ok(msg) = msg.to_text() {
                        if let Ok(can_frame) = CanFrame::from_json(msg) {
                            trace!("Get frame from WS: {:?}", &can_frame);
                            info!("Get frame from WS: {:?}", &can_frame);
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
        })
        .await;
}

async fn start_ws(addr: &str, in_tx: &Sender) {
    let listener = TcpListener::bind(addr)
        .await
        .expect(&format!("Can't bind websocket address {}", addr));

    info!("Listening on: {}", addr);

    if let Ok((stream, addr)) = listener.accept().await {
        handle_ws_connection(stream, addr, &in_tx).await;
    }
}

async fn start_can(can_addr: &str, ws_tx: Sender) {
    let mut socket_rx = CANSocket::open(can_addr).unwrap();
    tokio::spawn(async move {
        while let Some(Ok(frame)) = socket_rx.next().await {
            ws_tx.clone().send(a_frame().to_linux_frame()).await;
            println!("frame {:?}", frame);
        }
    })
    .await;
}

///         (in_tx    ,     in_rx)      = sync::mpsc::channel();
///         (out_tx    ,     out_rx)      = sync::mpsc::channel();
///     -> in_tx ----> in_rx ->
///     -> can_tx ----> ws_rx ->
///  WS                          CAN
///     <- ws_tx <---- can_rx <-
///     <- out_tx <---- out_rx <-
pub async fn run(ws_addr: String, can_addr: String) -> Result<(), Box<dyn std::error::Error>> {
    let (in_tx, mut in_rx) = sync::mpsc::channel(100);
    let (ws_tx, mut out_rx) = sync::mpsc::channel(100);

    let ws = start_ws(&ws_addr, &in_tx);
    let can = start_can(&can_addr, ws_tx);
    let mut socket_tx = CANSocket::open(&can_addr).unwrap();
    let ws_receiver = tokio::spawn(async move {
        while let Some(f) = in_rx.recv().await {
            info!("write can {:?}", &f);
            socket_tx.write_frame(f).unwrap().await;
        }
    });
    let can_receiver = tokio::spawn(async move {
        while let Some(f) = out_rx.recv().await {
            println!("received from can {:?}", f);
        }
    });

    tokio::join!(ws, ws_receiver, can);
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
