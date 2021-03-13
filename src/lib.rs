use futures_util::{future, pin_mut, stream::TryStreamExt, FutureExt, StreamExt, TryFutureExt};
use log::*;
use std::borrow::Borrow;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{self, oneshot};

type Sender<T = CanFrame> = sync::mpsc::Sender<T>;
type Receiver<T = CanFrame> = sync::mpsc::Receiver<T>;

#[derive(Debug)]
struct CanFrame {}

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
                    can_tx.clone().send(CanFrame {}).await;
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

///         (tx    ,     rx)      = sync::oneshot::channel();
///     -> can_tx ----> ws_rx ->
///  WS                          CAN
///     <- ws_tx <---- can_rx <-
pub async fn run(ws_addr: String) {
    let (can_tx, mut ws_rx) = sync::mpsc::channel(100);
    // let (ws_tx, can_rx) = sync::oneshot::channel();
    can_tx.clone().send(CanFrame {}).await;

    let ws = start_ws(&ws_addr, &can_tx);

    let rc = tokio::spawn(async move {
        while let Some(f) = ws_rx.recv().await {
            println!("go to can {:?}", f);
        }
    });
    tokio::join!(ws, rc);
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
