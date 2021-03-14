use ws2can::run;

#[tokio::main]
async fn main() {
    env_logger::init();
    run("127.0.0.1:8080".to_string(), "can0".to_string()).await;
    println!("Hello, world!");
}
