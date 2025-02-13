//! Server side portion of the tunneling implementation
//!
//! Details can be found on the GitHub issue: https://github.com/PocketRelay/Server/issues/64

use self::codec::{TunnelCodec, TunnelMessage};
use bytes::Bytes;
use futures_util::{Sink, Stream};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use log::error;
use parking_lot::RwLock;
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{ready, Context, Poll},
    time::Duration,
};
use tokio::{
    sync::mpsc,
    time::{interval_at, Instant, Interval, MissedTickBehavior},
};
use tokio_util::codec::Framed;

use crate::services::{sessions::AssociationId, tunnel::TunnelId};

use super::{
    mappings::{TunnelData, TunnelMappings},
    TunnelService,
};

/// The port bound on clients representing the host player within the socket pool
pub const TUNNEL_HOST_LOCAL_PORT: u16 = 42132;

#[derive(Default)]
pub struct HttpTunnelService {
    /// Underlying tunnel mappings
    pub mappings: RwLock<TunnelMappings<TunnelHandle>>,
}

/// Handle for sending messages to a tunnel
#[derive(Clone)]
pub struct TunnelHandle {
    /// The sender for sending messages to the tunnel
    tx: mpsc::UnboundedSender<TunnelMessage>,
}

/// Tunnel connection to a client
pub struct HttpTunnel {
    /// ID for this tunnel
    id: TunnelId,
    /// The IO tunnel used to send information to the host and receive
    /// response
    io: Framed<TokioIo<Upgraded>, TunnelCodec>,
    /// Receiver for messages that should be written to the tunnel
    rx: mpsc::UnboundedReceiver<TunnelMessage>,
    /// Future state for writing to the `io`
    write_state: TunnelWriteState,
    /// The service access
    service: Arc<TunnelService>,
    /// Interval for sending keep alive messages
    keep_alive_interval: Interval,
}

impl Drop for HttpTunnel {
    fn drop(&mut self) {
        // Remove the tunnel from the service
        self.service.dissociate_tunnel_http(self.id);
    }
}

/// Holds the state for the current writing progress for a [`Tunnel`]
#[derive(Default)]
enum TunnelWriteState {
    /// Waiting for a message to come through the [`Tunnel::rx`]
    #[default]
    Recv,
    /// Waiting for the [`Tunnel::io`] to be writable, then writing the
    /// contained [`TunnelMessage`]
    Write(Option<TunnelMessage>),
    /// Poll flushing the bytes written to [`Tunnel::io`]
    Flush,
    /// The tunnel has stopped and should not continue
    Stop,
}

/// Holds the state for the current reading progress for a [`Tunnel`]
enum TunnelReadState {
    /// Continue reading
    Continue,
    /// The tunnel has stopped and should not continue
    Stop,
}

impl HttpTunnel {
    // Send keep-alive pings every 10s
    const KEEP_ALIVE_DELAY: Duration = Duration::from_secs(10);

    /// Starts a new tunnel on `io` using the tunnel `service`
    ///
    /// ## Arguments
    /// * `service`     - The service to add the tunnel to
    /// * `association` - The client association ID for this tunnel
    /// * `io`          - The underlying tunnel IO
    pub fn start(service: Arc<TunnelService>, association: AssociationId, io: Upgraded) {
        let (tx, rx) = mpsc::unbounded_channel();

        // Wrap the `io` with the [`TunnelCodec`] for framing
        let io = Framed::new(TokioIo::new(io), TunnelCodec::default());

        // Store the tunnel mapping
        let tunnel_id = service.http.mappings.write().insert_tunnel(
            association,
            TunnelData {
                association,
                handle: TunnelHandle { tx },
                last_alive: Instant::now(),
            },
        );

        let tunnel_id = match tunnel_id {
            Some(value) => value,
            // Cannot allocate the tunnel an ID
            None => {
                error!("failed to allocate a tunnel ID: exhausted");
                return;
            }
        };

        // Create the interval to track keep alive pings
        let keep_alive_start = Instant::now() + Self::KEEP_ALIVE_DELAY;
        let mut keep_alive_interval = interval_at(keep_alive_start, Self::KEEP_ALIVE_DELAY);

        keep_alive_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        // Spawn the tunnel task
        tokio::spawn(HttpTunnel {
            service,
            id: tunnel_id,
            io,
            rx,
            write_state: Default::default(),
            keep_alive_interval,
        });
    }

    /// Polls accepting messages from [`Tunnel::rx`] then writing them to [`Tunnel::io`] and
    /// flushing the underlying stream. Provides the next [`TunnelWriteState`]
    /// when [`Poll::Ready`] is returned
    ///
    /// Should be repeatedly called until it no-longer returns [`Poll::Ready`]
    fn poll_write_state(&mut self, cx: &mut Context<'_>) -> Poll<TunnelWriteState> {
        Poll::Ready(match &mut self.write_state {
            TunnelWriteState::Recv => {
                // Try receive a packet from the write channel
                let result = ready!(Pin::new(&mut self.rx).poll_recv(cx));

                if let Some(message) = result {
                    TunnelWriteState::Write(Some(message))
                } else {
                    // All writers have closed, tunnel must be closed (Future end)
                    TunnelWriteState::Stop
                }
            }

            TunnelWriteState::Write(message) => {
                // Wait until the `io` is ready
                if ready!(Pin::new(&mut self.io).poll_ready(cx)).is_ok() {
                    let message = message
                        .take()
                        .expect("Unexpected write state without message");

                    // Write the packet to the buffer
                    Pin::new(&mut self.io)
                        .start_send(message)
                        // Packet encoder impl shouldn't produce errors
                        .expect("Message encoder errored");

                    TunnelWriteState::Flush
                } else {
                    // Failed to ready, tunnel must be closed
                    TunnelWriteState::Stop
                }
            }

            TunnelWriteState::Flush => {
                // Poll flushing `io`
                if ready!(Pin::new(&mut self.io).poll_flush(cx)).is_ok() {
                    TunnelWriteState::Recv
                } else {
                    // Failed to flush, tunnel must be closed
                    TunnelWriteState::Stop
                }
            }

            // Tunnel should *NOT* be polled if its already stopped
            TunnelWriteState::Stop => panic!("Tunnel polled after already stopped"),
        })
    }

    /// Polls reading messages from [`Tunnel::io`] and sending them to the correct
    /// handle within the [`Tunnel::pool`]. Provides the next [`TunnelReadState`]
    /// when [`Poll::Ready`] is returned
    ///
    /// Should be repeatedly called until it no-longer returns [`Poll::Ready`]
    fn poll_read_state(&mut self, cx: &mut Context<'_>) -> Poll<TunnelReadState> {
        // Try receive a message from the `io`
        let Some(Ok(mut message)) = ready!(Pin::new(&mut self.io).poll_next(cx)) else {
            // Cannot read next message stop the tunnel
            return Poll::Ready(TunnelReadState::Stop);
        };

        // Ping messages can be ignored
        if message.index == 255 {
            return Poll::Ready(TunnelReadState::Continue);
        }

        // Get the path through the tunnel
        let (target_handle, index) = {
            match self
                .service
                .http
                .mappings
                .read()
                .get_tunnel_route(self.id, message.index)
            {
                Some(value) => value,
                // Don't have a tunnel to send the message through
                None => return Poll::Ready(TunnelReadState::Continue),
            }
        };

        // Update the message target index to be from the correct index
        message.index = index;

        // Send the message to the tunnel
        _ = target_handle.tx.send(message);

        Poll::Ready(TunnelReadState::Continue)
    }
}

impl Future for HttpTunnel {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        // Poll the write half
        while let Poll::Ready(next_state) = this.poll_write_state(cx) {
            this.write_state = next_state;

            // Tunnel has stopped
            if let TunnelWriteState::Stop = this.write_state {
                return Poll::Ready(());
            }
        }

        // Poll the read half
        while let Poll::Ready(next_state) = this.poll_read_state(cx) {
            // Tunnel has stopped
            if let TunnelReadState::Stop = next_state {
                return Poll::Ready(());
            }
        }

        // Write a ping message at the interval if we aren't already sending a message
        if this.keep_alive_interval.poll_tick(cx).is_ready() {
            if let TunnelWriteState::Recv = this.write_state {
                // Move to a writing state
                this.write_state = TunnelWriteState::Write(Some(TunnelMessage {
                    index: 255,
                    message: Bytes::new(),
                }));

                // Poll the writer with the new message
                if let Poll::Ready(next_state) = this.poll_write_state(cx) {
                    this.write_state = next_state;

                    // Tunnel has stopped
                    if let TunnelWriteState::Stop = this.write_state {
                        return Poll::Ready(());
                    }
                }
            }
        }

        Poll::Pending
    }
}

mod codec {
    //! This modules contains the codec and message structures for [TunnelMessage]s
    //!
    //! # Tunnel Messages
    //!
    //! Tunnel message frames are as follows:
    //!
    //! ```norun
    //!  0                   1                   2                      
    //!  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3
    //! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    //! |     Index     |            Length             |
    //! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    //! |                                               :
    //! :                    Payload                    :
    //! :                                               |
    //! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    //! ```
    //!
    //! Tunnel message frames contain the following fields:
    //!
    //! Index: 8-bits. Determines the destination of the message within the current pool.
    //!
    //! Length: 16-bits. Determines the size in bytes of the payload that follows
    //!
    //! Payload: Variable length. The message bytes payload of `Length`
    //!
    //!
    //! ## Keep alive
    //!
    //! The server will send keep-alive messages, these are in the same
    //! format as the packet above. However, the index will always be 255
    //! and the payload will be empty.

    use bytes::{Buf, BufMut, Bytes};
    use tokio_util::codec::{Decoder, Encoder};

    /// Header portion of a [TunnelMessage] that contains the
    /// index of the message and the length of the expected payload
    struct TunnelMessageHeader {
        /// Socket index to use
        index: u8,
        /// The length of the tunnel message bytes
        length: u16,
    }

    /// Message sent through the tunnel
    pub struct TunnelMessage {
        /// Socket index to use
        pub index: u8,
        /// The message contents
        pub message: Bytes,
    }

    /// Codec for encoding and decoding tunnel messages
    #[derive(Default)]
    pub struct TunnelCodec {
        /// Stores the current message header while its waiting
        /// for the full payload to become available
        partial: Option<TunnelMessageHeader>,
    }

    impl Decoder for TunnelCodec {
        type Item = TunnelMessage;
        type Error = std::io::Error;

        fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
            let partial = match self.partial.as_mut() {
                Some(value) => value,
                None => {
                    // Not enough room for a partial frame
                    if src.len() < 5 {
                        return Ok(None);
                    }
                    let index = src.get_u8();
                    let length = src.get_u16();

                    self.partial.insert(TunnelMessageHeader { index, length })
                }
            };

            // Not enough data for the partial frame
            if src.len() < partial.length as usize {
                return Ok(None);
            }

            let partial = self.partial.take().expect("Partial frame missing");
            let bytes = src.split_to(partial.length as usize);

            Ok(Some(TunnelMessage {
                index: partial.index,
                message: bytes.freeze(),
            }))
        }
    }

    impl Encoder<TunnelMessage> for TunnelCodec {
        type Error = std::io::Error;

        fn encode(
            &mut self,
            item: TunnelMessage,
            dst: &mut bytes::BytesMut,
        ) -> Result<(), Self::Error> {
            dst.put_u8(item.index);
            dst.put_u16(item.message.len() as u16);
            dst.extend_from_slice(&item.message);
            Ok(())
        }
    }
}
