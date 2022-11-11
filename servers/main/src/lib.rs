use core::blaze::components::Components;
use core::blaze::session::{Session, SessionArc};
use core::{env, GlobalStateArc};

use blaze_pk::OpaquePacket;
use log::{debug, error, info};
use tokio::net::TcpListener;
use tokio::select;
use tokio::sync::mpsc;

mod routes;

/// Starts the Blaze server using the provided global state
/// which is cloned for the spawned sessions.
///
/// `global` The global state
pub async fn start_server(global: GlobalStateArc) {
    let listener = {
        let port = env::u16_env(env::MAIN_PORT);
        match TcpListener::bind(("0.0.0.0", port)).await {
            Ok(value) => {
                info!("Started Main Server on (Port: {port})");
                value
            }
            Err(err) => {
                error!("Failed to bind main server (Port: {}): {:?}", port, err);
                panic!();
            }
        }
    };

    let mut session_id = 1;
    let mut shutdown = global.shutdown.clone();
    loop {
        select! {
            result = listener.accept() => {
                match result {
                    Ok(values) => {

                        let (flush_send, flush_recv) = mpsc::channel(1);
                        let session = Session::new(global.clone(), session_id, values, flush_send);
                        tokio::spawn(process(session, flush_recv));
                        session_id += 1;
                    }
                    Err(err) => {
                        error!("Error occurred while accepting connections: {:?}", err);
                    }
                }
            }
            _ = shutdown.changed() => {
                info!("Stopping main server listener from shutdown trigger.");
                break;
            }
        }
    }
}

/// Processes the session by reading packets and flushing outbound content.
///
/// `session` The session to process
/// `flush`   The reciever for the flush messages
async fn process(session: SessionArc, mut flush: mpsc::Receiver<()>) {
    let mut shutdown = session.global.shutdown.clone();
    loop {
        select! {
            _ = flush.recv() => { session.flush().await; }
            result = session.read() => {
                if let Ok((component, packet)) = result {
                    process_packet(&session, component, &packet).await;
                } else {
                    break;
                }
            }
            _ = shutdown.changed() => {
                debug!("Shutting down session (SID: {})", session.id);
                break;
            }
        };
    }
    session.release().await;
}

/// Handles processing a recieved packet from the `process` function. This includes a
/// component for routing and the actual packet itself. The buffer is flushed after
/// routing is complete.
///
/// `session`   The session to process the packet for
/// `component` The component of the packet for routing
/// `packet`    The packet itself
async fn process_packet(session: &SessionArc, component: Components, packet: &OpaquePacket) {
    Session::debug_log_packet(session, "Read", packet).await;
    if let Err(err) = routes::route(session, component, packet).await {
        error!(
            "Error occurred while routing (SID: {}): {:?}",
            session.id, err
        );
    }
    session.flush().await;
}
