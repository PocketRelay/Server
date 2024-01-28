use log::warn;
use std::future::Future;
use std::pin::Pin;
use std::sync::{atomic::AtomicUsize, Arc};
use std::task::{ready, Context, Poll};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::PollSemaphore;

/// Lock with strict ordering for permits, maintains strict
/// FIFO ordering
#[derive(Clone)]
pub struct QueueLock {
    inner: Arc<QueueLockInner>,
}

impl QueueLock {
    pub fn new() -> QueueLock {
        let inner = QueueLockInner {
            semaphore: Arc::new(Semaphore::new(1)),
            next_ticket: AtomicUsize::new(1),
            current_ticket: AtomicUsize::new(1),
        };

        QueueLock {
            inner: Arc::new(inner),
        }
    }

    /// Acquire a ticket for the queue, returns a future
    /// which completes when its the tickets turn to access
    pub fn acquire(&self) -> TicketAcquireFuture {
        let ticket = self
            .inner
            .next_ticket
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        let poll = PollSemaphore::new(self.inner.semaphore.clone());

        TicketAcquireFuture {
            inner: self.inner.clone(),
            poll,
            ticket,
        }
    }
}

struct QueueLockInner {
    /// Underlying async acquisition primitive
    semaphore: Arc<Semaphore>,
    /// The next ticket to provide access to
    next_ticket: AtomicUsize,
    /// The current ticket allowed access
    current_ticket: AtomicUsize,
}

/// Future while waiting to acquire its lock
///
/// TODO: If these futures are dropped early then
/// the lock wont be able to unlock, figure out how
/// to fix this..?
pub struct TicketAcquireFuture {
    /// The queue lock being waited on
    inner: Arc<QueueLockInner>,
    /// Semaphore that can be polled
    poll: PollSemaphore,
    /// The ticket for this queue position
    ticket: usize,
}

impl Drop for TicketAcquireFuture {
    fn drop(&mut self) {
        let current = self
            .inner
            .current_ticket
            .load(std::sync::atomic::Ordering::SeqCst);

        // Ensure we are the ticket that is allowed
        if current != self.ticket {
            warn!("Early dropped ticket acquire {}", self.ticket);
        }
    }
}

/// Guard which releases the queue lock when dropped
pub struct QueueLockGuard {
    /// Acquisition permit
    _permit: OwnedSemaphorePermit,
    inner: Arc<QueueLockInner>,
}

impl Future for TicketAcquireFuture {
    type Output = QueueLockGuard;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let permit = ready!(this.poll.poll_acquire(cx)).expect("Queue task semaphore was closed");

        let current = this
            .inner
            .current_ticket
            .load(std::sync::atomic::Ordering::SeqCst);

        // Ensure we are the ticket that is allowed
        if current == this.ticket {
            Poll::Ready(QueueLockGuard {
                _permit: permit,
                inner: this.inner.clone(),
            })
        } else {
            // Make sure this future is polled again when possible
            // TODO: Is this okay to do?? (Tokio defers their version but thats internal crate access)
            cx.waker().wake_by_ref();

            Poll::Pending
        }
    }
}

impl Drop for QueueLockGuard {
    fn drop(&mut self) {
        // Set the current ticket to the next ticket
        self.inner
            .current_ticket
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    }
}
