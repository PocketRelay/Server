//! Sessions are client connections to the main server with associated
//! data such as player data for when they become authenticated and
//! networking data.

use self::{
    packet::{Packet, PacketCodec, PacketDebug},
    router::BlazeRouter,
};
use crate::{
    database::entities::Player,
    services::sessions::AssociationId,
    utils::components::{component_key, DEBUG_IGNORED_PACKETS},
};
use data::SessionData;
use futures_util::{future::BoxFuture, Sink, Stream};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use log::{debug, log_enabled, warn};
use std::{
    fmt::Debug,
    net::Ipv4Addr,
    pin::Pin,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    task::{ready, Context, Poll},
};
use std::{future::Future, sync::Weak};
use tokio::sync::{mpsc, OwnedMutexGuard};
use tokio_util::codec::Framed;

pub mod data;
pub mod models;
pub mod packet;
pub mod router;
pub mod routes;

pub type SessionLink = Arc<Session>;
pub type WeakSessionLink = Weak<Session>;

pub struct Session {
    /// Unique ID for this session
    id: u32,

    /// Lock for handling packets with a session, ensures only one packet is
    /// processed at a time and in the same order that it was received / sent
    busy_lock: Arc<tokio::sync::Mutex<()>>,

    /// Sender for sending packets to the session
    tx: mpsc::UnboundedSender<Packet>,

    /// Mutable data associated with the session
    pub data: SessionData,
}

#[derive(Clone)]
pub struct SessionNotifyHandle {
    busy_lock: Arc<tokio::sync::Mutex<()>>,
    tx: mpsc::UnboundedSender<Packet>,
}

impl SessionNotifyHandle {
    /// Pushes a new notification packet
    pub fn notify(&self, packet: Packet) {
        let tx = self.tx.clone();

        // Acquire the lock position before scheduling the task to ensure correct ordering
        let busy_lock = self.busy_lock.clone().lock_owned();

        tokio::spawn(async move {
            let _guard = busy_lock.await;
            let _ = tx.send(packet);
        });
    }
}

static SESSION_IDS: AtomicU32 = AtomicU32::new(1);

impl Session {
    pub async fn start(
        io: Upgraded,
        addr: Ipv4Addr,
        association: Option<AssociationId>,
        router: Arc<BlazeRouter>,
    ) {
        // Obtain a session ID
        let id = SESSION_IDS.fetch_add(1, Ordering::AcqRel);

        let (tx, rx) = mpsc::unbounded_channel();

        let session = Arc::new(Self {
            id,
            busy_lock: Default::default(),
            tx,
            data: SessionData::new(addr, association),
        });

        SessionFuture {
            io: Framed::new(TokioIo::new(io), PacketCodec::default()),
            router: &router,
            rx,
            session: session.clone(),
            read_state: ReadState::Recv,
            write_state: WriteState::Recv,
            stop: false,
        }
        .await;

        session.stop();
    }

    pub fn notify_handle(&self) -> SessionNotifyHandle {
        SessionNotifyHandle {
            busy_lock: self.busy_lock.clone(),
            tx: self.tx.clone(),
        }
    }

    /// Called when the session is considered stopped (Reader/Writer future has completed)
    /// in order to clean up any remaining references to the session before dropping
    fn stop(self: Arc<Self>) {
        // Clear session data
        self.data.clear();

        // Session should now be the sole owner
        let session = match Arc::try_unwrap(self) {
            Ok(value) => value,
            Err(arc) => {
                let references = Arc::strong_count(&arc);
                warn!(
                    "Failed to stop session {} there are still {} references to it",
                    arc.id, references
                );
                return;
            }
        };

        debug!("Session stopped (SID: {})", session.id);
    }

    /// Logs the contents of the provided packet to the debug output along with
    /// the header information and basic session information.
    ///
    /// `action` The name of the action this packet is undergoing.
    ///          (e.g. Writing or Reading)
    /// `packet` The packet that is being logged
    fn debug_log_packet(&self, action: &'static str, packet: &Packet) {
        // Skip if debug logging is disabled
        if !log_enabled!(log::Level::Debug) {
            return;
        }

        let key = component_key(packet.frame.component, packet.frame.command);

        // Don't log the packet if its debug ignored
        if DEBUG_IGNORED_PACKETS.contains(&key) {
            return;
        }

        // Get the authenticated player to include in the debug message
        let auth = self.data.get_player();

        let debug_data = DebugSessionData {
            action,
            id: self.id,
            auth,
        };
        let debug_packet = PacketDebug { packet };

        debug!("\n{:?}{:?}", debug_data, debug_packet);
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
    /// The IO for reading and writing
    io: Framed<TokioIo<Upgraded>, PacketCodec>,
    /// Receiver for packets to write
    rx: mpsc::UnboundedReceiver<Packet>,
    /// The session this link is for
    session: SessionLink,
    /// The router to use
    router: &'a BlazeRouter,
    /// The reading state
    read_state: ReadState<'a>,
    /// The writing state
    write_state: WriteState,
    /// Whether the future has been stopped
    stop: bool,
}

/// Session future writing state
enum WriteState {
    /// Waiting for a packet to write
    Recv,
    /// Waiting for the framed to become read
    Write { packet: Option<Packet> },
    /// Flushing the framed
    Flush,
}

/// Session future reading state
enum ReadState<'a> {
    /// Waiting for a packet
    Recv,
    /// Acquiring a lock guard
    Acquire {
        /// Future for the locking guard
        lock_future: BoxFuture<'static, OwnedMutexGuard<()>>,
        /// The packet that was read
        packet: Option<Packet>,
    },
    /// Future for a handler is being polled
    Handle {
        /// Locking guard
        guard: OwnedMutexGuard<()>,
        /// Handle future
        future: BoxFuture<'a, Packet>,
    },
}

impl SessionFuture<'_> {
    /// Polls the write state, the poll ready state returns whether
    /// the future should continue
    fn poll_write_state(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        match &mut self.write_state {
            WriteState::Recv => {
                // Try receive a packet from the write channel
                let result = ready!(Pin::new(&mut self.rx).poll_recv(cx));

                if let Some(packet) = result {
                    self.write_state = WriteState::Write {
                        packet: Some(packet),
                    };
                } else {
                    // All writers have closed, session must be closed (Future end)
                    self.stop = true;
                }
            }
            WriteState::Write { packet } => {
                // Wait until the inner is ready
                if ready!(Pin::new(&mut self.io).poll_ready(cx)).is_ok() {
                    let packet = packet
                        .take()
                        .expect("Unexpected write state without packet");

                    self.session.debug_log_packet("Send", &packet);

                    // Write the packet to the buffer
                    Pin::new(&mut self.io)
                        .start_send(packet)
                        // Packet encoder impl shouldn't produce errors
                        .expect("Packet encoder errored");

                    self.write_state = WriteState::Flush;
                } else {
                    // Failed to ready, session must be closed
                    self.stop = true;
                }
            }
            WriteState::Flush => {
                // Wait until the flush is complete
                if ready!(Pin::new(&mut self.io).poll_flush(cx)).is_ok() {
                    self.write_state = WriteState::Recv;
                } else {
                    // Failed to flush, session must be closed
                    self.stop = true
                }
            }
        }

        Poll::Ready(())
    }

    /// Polls the read state, the poll ready state returns whether
    /// the future should continue
    fn poll_read_state(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        match &mut self.read_state {
            ReadState::Recv => {
                // Try receive a packet from the write channel
                let result = ready!(Pin::new(&mut self.io).poll_next(cx));

                if let Some(Ok(packet)) = result {
                    let lock_future = self.session.busy_lock.clone().lock_owned();
                    let lock_future: BoxFuture<'static, OwnedMutexGuard<()>> =
                        Box::pin(lock_future);

                    self.read_state = ReadState::Acquire {
                        lock_future,
                        packet: Some(packet),
                    }
                } else {
                    // Reader has closed or reading encountered an error (Either way stop reading)
                    self.stop = true;
                }
            }
            ReadState::Acquire {
                lock_future,
                packet,
            } => {
                let guard = ready!(Pin::new(lock_future).poll(cx));
                let packet = packet
                    .take()
                    .expect("Unexpected acquire state without packet");

                self.session.debug_log_packet("Receive", &packet);

                let future = self.router.handle(self.session.clone(), packet);

                // Move onto a handling state
                self.read_state = ReadState::Handle { guard, future };
            }
            ReadState::Handle {
                guard: _guard,
                future,
            } => {
                // Poll the handler until completion
                let response = ready!(Pin::new(future).poll(cx));

                // Send the response to the writer
                _ = self.session.tx.send(response);

                // Reset back to the reading state
                self.read_state = ReadState::Recv;
            }
        }
        Poll::Ready(())
    }
}

impl Future for SessionFuture<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        while this.poll_write_state(cx).is_ready() {}
        while this.poll_read_state(cx).is_ready() {}

        if this.stop {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
