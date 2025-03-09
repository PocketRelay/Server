use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{ready, Context, Poll},
};

use futures_util::{future::BoxFuture, SinkExt, StreamExt};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use tokio::sync::{mpsc, Mutex, OwnedMutexGuard};
use tokio_util::codec::Framed;

use super::packet::{Packet, PacketCodec};

/// Future for processing a blaze socket
pub struct BlazeSocketFuture {
    /// Socket being acted upon
    io: Framed<TokioIo<Upgraded>, PacketCodec>,
    /// Channel for processing received messages
    inbound_tx: Option<mpsc::UnboundedSender<Packet>>,
    /// Channel for outbound messages
    outbound_rx: mpsc::UnboundedReceiver<Packet>,
    /// Currently accepted outbound item, ready to be written
    buffered_item: Option<Packet>,
}

/// Blaze message response sending is locked by a write lock
///
/// This is because when handling messages the write lock must
/// be held at the start in order to first write the response
/// message for a handler before writing any notification
/// messages
#[derive(Clone)]
pub struct BlazeTx {
    tx: Arc<Mutex<mpsc::UnboundedSender<Packet>>>,
}

pub type BlazeLockFuture = BoxFuture<'static, BlazeLock>;
pub type BlazeLock = OwnedMutexGuard<mpsc::UnboundedSender<Packet>>;

impl BlazeTx {
    /// Pushes a new notification packet
    pub fn notify(&self, packet: Packet) {
        // Acquire the lock position before scheduling the task to ensure correct ordering
        let tx = self.acquire_tx();

        tokio::spawn(async move {
            let tx = tx.await;
            let _ = tx.send(packet);
        });
    }

    /// Create a future to acquire the sender lock.
    pub fn acquire_tx(&self) -> impl Future<Output = BlazeLock> + 'static {
        self.tx.clone().lock_owned()
    }
}

pub type BlazeRx = mpsc::UnboundedReceiver<Packet>;

impl BlazeSocketFuture {
    pub fn new(
        io: Framed<TokioIo<Upgraded>, PacketCodec>,
    ) -> (BlazeSocketFuture, BlazeRx, BlazeTx) {
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
        let (outbound_tx, outbound_rx) = mpsc::unbounded_channel();

        let future = BlazeSocketFuture {
            io,
            inbound_tx: Some(inbound_tx),
            outbound_rx,
            buffered_item: None,
        };

        let outbound_tx = BlazeTx {
            tx: Arc::new(Mutex::new(outbound_tx)),
        };

        (future, inbound_rx, outbound_tx)
    }
}

impl Future for BlazeSocketFuture {
    type Output = Result<(), std::io::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        // Read messages from the socket
        while let Some(inbound_tx) = &mut this.inbound_tx {
            let msg = match this.io.poll_next_unpin(cx) {
                Poll::Ready(Some(result)) => result?,

                // Socket is already closed, cannot ready anything more
                Poll::Ready(None) => return Poll::Ready(Ok(())),

                // Nothing yet, move onto the write polling
                Poll::Pending => break,
            };

            if inbound_tx.send(msg).is_err() {
                // Receiver for messages has dropped, stop reading messages
                this.inbound_tx.take();
                break;
            }
        }

        // Write messages to the socket
        loop {
            if this.buffered_item.is_some() {
                // Wait until the socket is ready
                ready!(this.io.poll_ready_unpin(cx))?;

                // Take the buffered item
                let packet = this
                    .buffered_item
                    .take()
                    .expect("unexpected write state without a packet");

                // Write the buffered item
                this.io.start_send_unpin(packet)?;
            }

            match this.outbound_rx.poll_recv(cx) {
                // Message ready, set the buffered item
                Poll::Ready(Some(item)) => {
                    this.buffered_item = Some(item);
                }
                // All message senders have dropped, close the socket
                Poll::Ready(None) => {
                    ready!(this.io.poll_close_unpin(cx))?;
                    return Poll::Ready(Ok(()));
                }
                Poll::Pending => {
                    // Failed to flush the socket
                    ready!(this.io.poll_flush_unpin(cx))?;
                    return Poll::Pending;
                }
            }
        }
    }
}
