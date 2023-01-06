//! Module for the Redirector server which handles redirecting the clients
//! to the correct address for the main server.

use crate::env;
use log::{debug, error, info};
use tokio::{
    io::AsyncReadExt,
    net::{TcpListener, TcpStream},
};

pub async fn start_server() {
    // Initializing the underlying TCP listener
    let listener = {
        let port = env::from_env(env::TICKER_PORT);
        match TcpListener::bind(("0.0.0.0", port)).await {
            Ok(value) => {
                info!("Started Ticker server (Port: {})", port);
                value
            }
            Err(_) => {
                error!("Failed to bind Ticker server (Port: {})", port);
                panic!()
            }
        }
    };

    // Accept incoming connections
    loop {
        let stream: TcpStream = match listener.accept().await {
            Ok((stream, _)) => stream,
            Err(err) => {
                error!("Failed to accept ticker connection: {err:?}");
                continue;
            }
        };
        debug!("ACCEPTED TICKER CLIENT");
        tokio::spawn(async move {
            let mut stream = stream;
            // Buffer for reading data
            let mut buffer = [0u8; 1024];
            while let Ok(count) = stream.read(&mut buffer).await {
                if count == 0 {
                    break;
                }
                let slice = &buffer[..count];
                debug!("[TICKER] {:?}", slice)
            }
        });
    }
}
