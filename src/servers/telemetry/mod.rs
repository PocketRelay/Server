//! Module for the Redirector server which handles redirecting the clients
//! to the correct address for the main server.

use crate::state::GlobalState;
use log::{debug, error, info};
use std::io;
use tokio::{
    io::AsyncReadExt,
    net::{TcpListener, TcpStream},
};

/// Starts the telemetry server which collects telemetry messages from the
/// game clients and logs them to the debug log
pub async fn start_server() {
    // Initializing the underlying TCP listener
    let listener = {
        let config = GlobalState::config();
        let port = config.ports.telemetry;
        match TcpListener::bind(("0.0.0.0", port)).await {
            Ok(value) => {
                info!("Started Telemetry server (Port: {})", port);
                value
            }
            Err(_) => {
                error!("Failed to bind Telemetry server (Port: {})", port);
                panic!()
            }
        }
    };

    // Accept incoming connections
    loop {
        let stream: TcpStream = match listener.accept().await {
            Ok((stream, _)) => stream,
            Err(err) => {
                error!("Failed to accept telemetry connection: {err:?}");
                continue;
            }
        };

        tokio::spawn(async move {
            let mut stream = stream;
            while let Ok(message) = read_message(&mut stream).await {
                let message: TelemetryMessage = decode_message(message);
                debug!("[TELEMETRY] {:?}", message);
            }
        });
    }
}

/// Reads a telemetry message buffer from the provided input
/// stream returning the buffer that was read.
///
/// `stream` The stream to read from
async fn read_message(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let length = {
        // Buffer for reading the header + padding + legnth bytes
        let mut header = [0u8; 12];
        stream.read_exact(&mut header).await?;
        let mut bytes = [0u8; 2];
        bytes.copy_from_slice(&header[10..]);
        u16::from_be_bytes(bytes)
    };

    // Remove the header size from the message length
    let length = (length - 12.min(length)) as usize;

    // Create a new buffer of the expected size
    let mut buffer = vec![0u8; length];
    stream.read_exact(&mut buffer).await?;
    Ok(buffer)
}

// Structure containing key value pairs for telemetry messages
#[derive(Debug)]
pub struct TelemetryMessage {
    // Vec of key values
    pub values: Vec<(String, String)>,
}

/// Decodes the telemetry message from the message buffer into
/// a telemetry message structure
///
/// `message` The raw message bytes
fn decode_message(mut message: Vec<u8>) -> TelemetryMessage {
    // Split the buffer into pairs of values
    let pairs = message
        .split_mut(|value| b'\n'.eq(value))
        .filter_map(|slice| split_at_byte(slice, b'='));

    let mut values = Vec::new();

    for (key, value) in pairs {
        let key = String::from_utf8_lossy(key);
        let value = if key.eq("TLM3") {
            decode_tlm3(value)
        } else {
            format!("{:?}", value)
        };
        values.push((key.to_string(), value))
    }

    TelemetryMessage { values }
}

/// TLM3 key for decoding the TML3 line
const TLM3_KEY: &[u8] = b"The truth is back in style.";

/// Splits the provided bytes slice at the first of the provided
/// byte returning None if there was no match and a slice before
/// and after if there is one
///
/// `value` The slice to split
/// `split` The byte to split at
fn split_at_byte(value: &mut [u8], split: u8) -> Option<(&mut [u8], &mut [u8])> {
    let mut parts = value.splitn_mut(2, |value| split.eq(value));
    let first = parts.next()?;
    let second = parts.next()?;
    Some((first, second))
}

/// Decodes a TLM3 line from the provided slice. Decodes in place
/// using a mutable slice of the value
///
/// `slice` The slice to decode from
fn decode_tlm3(slice: &mut [u8]) -> String {
    if let Some((_, line)) = split_at_byte(slice, b'-') {
        let mut out = String::new();
        for i in 0..line.len() {
            let value = line[i];
            let key_value = TLM3_KEY[i % TLM3_KEY.len()];

            let char = if (value ^ key_value) <= 0x80 {
                value ^ key_value
            } else {
                key_value ^ (value - 0x80)
            } as char;
            out.push(char);
        }
        out
    } else {
        format!("{slice:?}")
    }
}
