use std::net::SocketAddr;
use std::result::Result::Ok;

use futures_util::{future, pin_mut, SinkExt, StreamExt};
use log::*;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync;
use tokio_socketcan::{CANFrame, CANSocket};
use tokio_tungstenite::tungstenite::Message;

use can::CanFrame;

mod can;

type Sender<T = CANFrame> = sync::mpsc::Sender<T>;
type Receiver<T = CANFrame> = sync::mpsc::Receiver<T>;

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
                        if let Err(e) = in_tx.clone().send(can_frame.to_linux_frame()).await {
                            error!(
                                "Error occurred while sending frame from WS to CAN channel: {:?}",
                                e
                            );
                        }
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
            if let Err(e) = outgoing.send(msg).await {
                error!("Error occurred while sending WS message: {:?}", e);
            }
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

async fn start_can(can_addr: &str, out_tx: &Sender, mut in_rx: Receiver) {
    let can_socket = CANSocket::open(&can_addr).unwrap();
    let (mut outgoing, incoming) = can_socket.split();

    let can_receiver = incoming.for_each(|msg| async move {
        match msg {
            Ok(msg) => {
                info!("CAN(in): {:?}", msg);
                if let Err(e) = out_tx.clone().send(msg).await {
                    error!(
                        "Error occurred while sending frame from CAN to WS channel: {:?}",
                        e
                    );
                }
            }
            Err(e) => {
                error!("Error occurred while CAN receiving a message: {:?}", e);
            }
        }
    });

    let can_transmitter = tokio::spawn(async move {
        while let Some(frame) = in_rx.recv().await {
            info!("CAN(out): {:?}", frame);
            if let Err(e) = outgoing.send(frame).await {
                error!("Error occurred while sending CAN frame: {:?}", e);
            }
        }
    });

    if let (Err(e), _) = tokio::join!(can_transmitter, can_receiver) {
        error!("Error occurred in can_transmitter task: {:?}", e);
    }
}

///       (in_tx   ,   in_rx )      = sync::mpsc::channel();
///       (out_tx  ,   out_rx)      = sync::mpsc::channel();
///     -> in_tx ----> in_rx ->
///  WS                          CAN
///     <- out_tx <---- out_rx <-
pub async fn run(ws_addr: &str, can_addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    let (in_tx, in_rx) = sync::mpsc::channel(100);
    let (out_tx, out_rx) = sync::mpsc::channel(100);

    let ws = start_ws(&ws_addr, &in_tx, out_rx);
    let can = start_can(&can_addr, &out_tx, in_rx);

    tokio::join!(ws, can);

    Ok(())
}
