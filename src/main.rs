use std::env;
use std::io;

use ws2can::run;

mod can;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        panic!(print_usage());
    }

    run(args[1].as_str(), args[2].as_str()).await
}

fn print_usage() {
    println!(
        "
Not enough arguments, usage:
ws2can websocket_ip:port can_device
example:
ws2can 192.168.0.20:8080 can0
    "
    );
}
