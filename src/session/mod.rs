//! Sessions are client connections to the main server with associated
//! data such as player data for when they become authenticated and
//! networking data.

use self::{
    packet::{Packet, PacketCodec, PacketDebug},
    router::BlazeRouter,
};
use crate::{
    database::entities::Player,
    utils::components::{component_key, DEBUG_IGNORED_PACKETS},
};
use blaze_socket::{BlazeLock, BlazeLockFuture, BlazeRx, BlazeSocketFuture, BlazeTx};
use data::SessionData;
use futures_util::future::BoxFuture;
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use log::{debug, error, log_enabled};
use std::{
    fmt::Debug,
    pin::Pin,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    task::{ready, Context, Poll},
};
use std::{future::Future, sync::Weak};
use tokio::spawn;
use tokio_util::codec::Framed;

pub mod blaze_socket;
pub mod data;
pub mod models;
pub mod packet;
pub mod router;
pub mod routes;

pub type SessionLink = Arc<Session>;
pub type WeakSessionLink = Weak<Session>;

static SESSION_IDS: AtomicU32 = AtomicU32::new(1);

pub struct Session {
    /// Unique ID for this session
    pub id: u32,

    /// Handle for sending packets to this session
    pub tx: BlazeTx,

    /// Data associated with the session
    pub data: SessionData,
}

impl Session {
    /// Get an ID for a session
    pub fn acquire_id() -> u32 {
        SESSION_IDS.fetch_add(1, Ordering::AcqRel)
    }

    pub fn run(id: u32, io: Upgraded, data: SessionData, router: Arc<BlazeRouter>) {
        // Create blaze socket handler
        let (blaze_future, blaze_rx, blaze_tx) =
            BlazeSocketFuture::new(Framed::new(TokioIo::new(io), PacketCodec::default()));

        spawn(async move {
            if let Err(cause) = blaze_future.await {
                error!("error running blaze socket future: {cause:?}")
            }

            debug!("session blaze future completed");
        });

        debug!("session started (SID: {id})");

        // Create session handler
        let session = Arc::new(Self {
            id,
            tx: blaze_tx,
            data,
        });

        spawn({
            let session = session;

            async move {
                // Run the session to completion
                SessionFuture::new(blaze_rx, &session, &router).await;

                debug!("session future complete");

                // Clear session data, speeds up process of ending the session
                // prevents session data being accessed while shutting down
                session.data.clear_auth();

                debug!("session auth cleared, session dropped");
            }
        });
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        debug!("Session stopped (SID: {})", self.id);
    }
}

struct DebugSessionData {
    id: u32,
    auth: Option<Arc<Player>>,
    action: &'static str,
}

impl Debug for DebugSessionData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Session ({}): {}", self.id, self.action)?;

        if let Some(auth) = &self.auth {
            writeln!(f, "Auth ({}): (Name: {})", auth.id, &auth.display_name,)?;
        }

        Ok(())
    }
}

/// Future for processing a session
struct SessionFuture<'a> {
    /// Receiver for packets to handle
    rx: BlazeRx,
    /// The session this link is for
    session: &'a SessionLink,
    /// The router to use
    router: &'a BlazeRouter,
    /// State of the future
    state: SessionFutureState<'a>,
}

/// Session future reading state
enum SessionFutureState<'a> {
    /// Waiting for inbound packet
    Accept,
    /// Waiting to acquire write handling lock
    Acquire {
        /// Future for the locking guard
        lock_future: BlazeLockFuture,
        /// The packet that was read
        packet: Option<Packet>,
    },
    /// Future for a handler is being polled
    Handle {
        /// Access to the sender for sending the response
        tx: BlazeLock,
        /// Handle future
        future: BoxFuture<'a, Packet>,
    },
}

impl SessionFuture<'_> {
    pub fn new<'a>(
        rx: BlazeRx,
        session: &'a Arc<Session>,
        router: &'a BlazeRouter,
    ) -> SessionFuture<'a> {
        SessionFuture {
            router,
            rx,
            session,
            state: SessionFutureState::Accept,
        }
    }
}

impl Future for SessionFuture<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        loop {
            // Poll checking if the connection has timed-out
            if this.session.data.poll_keep_alive_dead(cx) {
                return Poll::Ready(());
            }

            match &mut this.state {
                SessionFutureState::Accept => {
                    let packet = match ready!(this.rx.poll_recv(cx)) {
                        Some(value) => value,
                        None => {
                            // Read half of the socket has terminated, nothing left to handle
                            return Poll::Ready(());
                        }
                    };

                    // Acquire a write lock future (Reserve our space for sending the response)
                    let lock_future = Box::pin(this.session.tx.acquire_tx());

                    this.state = SessionFutureState::Acquire {
                        lock_future,
                        packet: Some(packet),
                    }
                }
                SessionFutureState::Acquire {
                    lock_future,
                    packet,
                } => {
                    let guard = ready!(Pin::new(lock_future).poll(cx));
                    let packet = packet
                        .take()
                        .expect("Unexpected acquire state without packet");

                    debug_log_packet(this.session, "Receive", &packet);

                    let future = this.router.handle(this.session.clone(), packet);

                    // Move onto a handling state
                    this.state = SessionFutureState::Handle { tx: guard, future };
                }
                SessionFutureState::Handle { tx, future } => {
                    // Poll the handler until completion
                    let response = ready!(Pin::new(future).poll(cx));

                    // Send the response to the writer
                    if tx.send(response).is_err() {
                        // Write half has closed, cease reading
                        return Poll::Ready(());
                    }

                    // Reset back to the reading state
                    this.state = SessionFutureState::Accept;
                }
            }
        }
    }
}

/// Logs debugging information about a player
fn debug_log_packet(session: &Session, action: &'static str, packet: &Packet) {
    // Skip if debug logging is disabled
    if !log_enabled!(log::Level::Debug) {
        return;
    }

    let key = component_key(packet.frame.component, packet.frame.command);

    // Don't log the packet if its debug ignored
    if DEBUG_IGNORED_PACKETS.contains(&key) {
        return;
    }

    let id = session.id;
    let auth = session.data.get_player();

    let debug_data = DebugSessionData { action, id, auth };
    let debug_packet = PacketDebug { packet };

    debug!("\n{:?}{:?}", debug_data, debug_packet);
}
