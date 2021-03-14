use can::CanFrame;
use futures_util::{future, pin_mut, stream::TryStreamExt, FutureExt, StreamExt, TryFutureExt};
use log::*;
use std::borrow::Borrow;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{self, oneshot};
use tokio_socketcan::{CANSocket, Error};

type Sender<T = CanFrame> = sync::mpsc::Sender<T>;
type Receiver<T = CanFrame> = sync::mpsc::Receiver<T>;

mod can;

fn a_frame() -> CanFrame {
    CanFrame::new(0x123, [3; 8], 5, false, false)
}

async fn handle_ws_connection(raw_stream: TcpStream, addr: SocketAddr, can_tx: &Sender) {
    info!("Incoming TCP connection from: {}", addr);

    let stream = tokio_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");
    info!("WebSocket connection established: {}", addr);

    let (outgoing, incoming) = stream.split();

    incoming
        .for_each(|msg| async move {
            // info!("Received WS message: {}", msg.to_text().unwrap());
            match msg {
                Ok(msg) => {
                    info!("mes {}", msg);
                    can_tx.clone().send(a_frame()).await;
                }
                Err(e) => {
                    error!("Error occurred while WS receiving a message: {:?}", e);
                }
            }
        })
        .await;
}

async fn start_ws(addr: &str, can_tx: &Sender) {
    let listener = TcpListener::bind(addr)
        .await
        .expect(&format!("Can't bind websocket address {}", addr));

    info!("Listening on: {}", addr);

    if let Ok((stream, addr)) = listener.accept().await {
        handle_ws_connection(stream, addr, &can_tx).await;
    }
}

async fn start_can(can_addr: &str, ws_tx: Sender) {
    let mut socket_rx = CANSocket::open(can_addr).unwrap();
    tokio::spawn(async move {
        while let Some(Ok(frame)) = socket_rx.next().await {
            ws_tx.clone().send(a_frame()).await;
            println!("frame {:?}", frame);
        }
    })
    .await;
}

///         (tx    ,     rx)      = sync::oneshot::channel();
///     -> can_tx ----> ws_rx ->
///  WS                          CAN
///     <- ws_tx <---- can_rx <-
pub async fn run(ws_addr: String, can_addr: String) {
    let (can_tx, mut ws_rx) = sync::mpsc::channel(100);
    let (ws_tx, mut can_rx) = sync::mpsc::channel(100);

    let ws = start_ws(&ws_addr, &can_tx);
    let can = start_can(&can_addr, ws_tx);
    let ws_receiver = tokio::spawn(async move {
        while let Some(f) = ws_rx.recv().await {
            println!("go to can {:?}", f);
        }
    });
    let can_receiver = tokio::spawn(async move {
        while let Some(f) = can_rx.recv().await {
            println!("received from can {:?}", f);
        }
    });

    tokio::join!(ws, ws_receiver, can);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

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
