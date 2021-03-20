use std::io;

use ws2can::run;

// use ws::run;
mod can;
// mod ws;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    // run("127.0.0.1:8080", "can0").await
    run("192.168.0.28:8080".to_string(), "can0".to_string()).await
}
