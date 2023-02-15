use std::marker::PhantomData;

use tokio::sync::{mpsc, oneshot};

pub trait Actor: Sized + Send + 'static {
    // Unique ID for the actor provided to the address type
    fn id(&self) -> u32;

    fn started(&mut self, _ctx: &mut ActorContext<Self>) {}

    fn create<F>(action: F, id: u32) -> Addr<Self>
    where
        F: FnOnce(&mut ActorContext<Self>) -> Self,
    {
        let (tx, rx) = mpsc::unbounded_channel();
        let addr = Addr { tx, id };
        let mut ctx = ActorContext {
            rx,
            addr: addr.clone(),
        };
        let this = action(&mut ctx);
        this.spawn(ctx);
        addr
    }

    fn start(self) -> Addr<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        let addr = Addr { tx, id: self.id() };
        let ctx = ActorContext {
            rx,
            addr: addr.clone(),
        };

        self.spawn(ctx);
        addr
    }

    fn spawn(self, mut ctx: ActorContext<Self>) {
        tokio::spawn(async move {
            let mut this = self;
            this.started(&mut ctx);
            ctx.process(&mut this).await;
            this.stopping();
        });
    }

    fn stopping(&mut self) {}
}

pub trait Message: Send + 'static {
    type Result: Send + 'static;
}

pub trait Handler<M: Message>: Actor {
    fn handle(&mut self, msg: M, ctx: &mut ActorContext<Self>) -> M::Result;
}

pub struct ActorContext<A: Actor> {
    rx: mpsc::UnboundedReceiver<Box<dyn EnvelopeProxy<A>>>,
    /// Storage for maintaining an
    addr: Addr<A>,
}

/// Message used for shutting down an actor
struct StopMessage;

impl Message for StopMessage {
    type Result = ();
}

impl<A> EnvelopeProxy<A> for StopMessage
where
    A: Actor,
{
    fn handle(self: Box<Self>, _actor: &mut A, _ctx: &mut ActorContext<A>) -> Action {
        Action::Stop
    }
}

enum Action {
    Continue,
    Stop,
}

impl<A> ActorContext<A>
where
    A: Actor,
{
    async fn process(&mut self, actor: &mut A) {
        while let Some(msg) = self.rx.recv().await {
            let result = msg.handle(actor, self);
            match result {
                Action::Stop => break,
                Action::Continue => continue,
            }
        }
    }

    pub fn addr(&mut self) -> Addr<A> {
        self.addr.clone()
    }
}

/// Trait implemented by something that can be used as
/// an address for sending messages to its actor
pub struct Addr<A: Actor> {
    pub id: u32,
    tx: mpsc::UnboundedSender<Box<dyn EnvelopeProxy<A>>>,
}

impl<A> Clone for Addr<A>
where
    A: Actor,
{
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tx: self.tx.clone(),
        }
    }
}

/// Errors that can occur within the address
/// process
pub enum AddrError {
    /// Failed to send the message to the actor
    Send,
    /// Failed to receive the response from the actor
    Recv,
}

impl<A> Addr<A>
where
    A: Actor,
{
    /// Sends a message to the connected actor and waits for
    /// a response from the actor
    ///
    /// `msg` The message to send to the actor
    pub async fn send<M, R>(&self, msg: M) -> Result<R, AddrError>
    where
        A: Handler<M>,
        M: Message<Result = R>,
        R: Send + 'static,
    {
        let (tx, rx) = oneshot::channel();

        if self.tx.send(Box::new(Envelope { msg, tx })).is_err() {
            return Err(AddrError::Send);
        }

        rx.await.map_err(|_| AddrError::Recv)
    }

    /// Sends a message to the connected actor without
    /// waiting for a response returns whether the message
    /// was able to be sent
    pub fn do_send<M>(&self, msg: M) -> bool
    where
        A: Handler<M>,
        M: Message,
    {
        self.tx.send(Box::new(DiscardEnvelope { msg })).is_ok()
    }

    /// Sends a action to the actor for the actor to execute
    /// on itself handles the result of the action and returns it
    ///
    /// `msg` The message to send to the actor
    pub async fn exec<F, R>(&self, action: F) -> Result<R, AddrError>
    where
        F: FnOnce(&mut A, &mut ActorContext<A>) -> R + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = oneshot::channel();

        if self
            .tx
            .send(Box::new(ActionEnvelope {
                action,
                tx,
                _marker: PhantomData,
            }))
            .is_err()
        {
            return Err(AddrError::Send);
        }

        rx.await.map_err(|_| AddrError::Recv)
    }

    /// Sends a action to the actor for the actor to execute
    /// on itself handles the result of the action and returns it
    ///
    /// `msg` The message to send to the actor
    pub fn do_exec<F, R>(&self, action: F) -> bool
    where
        F: FnOnce(&mut A, &mut ActorContext<A>) -> R + Send + 'static,
        R: Send + 'static,
    {
        self.tx
            .send(Box::new(DiscardActionEnvelope {
                action,
                _marker: PhantomData,
            }))
            .is_ok()
    }

    /// Sends a stop message to the actor to tell it to
    /// stop processing any further messages and drop
    pub fn stop(&self) {
        self.tx.send(Box::new(StopMessage)).ok();
    }
}

trait EnvelopeProxy<A: Actor>: Send {
    fn handle(self: Box<Self>, actor: &mut A, ctx: &mut ActorContext<A>) -> Action;
}

struct Envelope<M, R> {
    /// The message contained within the envelope
    msg: M,
    /// Sender for sending the result of the message
    tx: oneshot::Sender<R>,
}

impl<A, M, R> EnvelopeProxy<A> for Envelope<M, R>
where
    A: Actor + Handler<M>,
    M: Message<Result = R>,
    R: Send + 'static,
{
    fn handle(self: Box<Self>, actor: &mut A, ctx: &mut ActorContext<A>) -> Action {
        let result = actor.handle(self.msg, ctx);
        self.tx.send(result).ok();
        Action::Continue
    }
}

struct DiscardEnvelope<M> {
    /// The message contained within the envelope
    msg: M,
}

impl<A, M> EnvelopeProxy<A> for DiscardEnvelope<M>
where
    A: Actor + Handler<M>,
    M: Message,
{
    fn handle(self: Box<Self>, actor: &mut A, ctx: &mut ActorContext<A>) -> Action {
        actor.handle(self.msg, ctx);
        Action::Continue
    }
}

/// Trait representing an action that can be executed
/// using the session
pub trait ActorAction<A: Actor>: Sized + Send + 'static {
    /// Type for the resulting value created from this action
    type Result: Send + 'static;

    fn handle(self, actor: &mut A, ctx: &mut ActorContext<A>) -> Self::Result;
}

impl<Act, F, R> ActorAction<Act> for F
where
    Act: Actor,
    F: FnOnce(&mut Act, &mut ActorContext<Act>) -> R + Send + 'static,
    R: Send + 'static,
{
    type Result = R;

    fn handle(self, actor: &mut Act, ctx: &mut ActorContext<Act>) -> Self::Result {
        self(actor, ctx)
    }
}

struct ActionEnvelope<Act: Actor, A, R> {
    /// The action to execute
    action: A,
    /// Sender for sending the result of the action
    tx: oneshot::Sender<R>,
    /// Marker for storing the actor type
    _marker: PhantomData<Act>,
}

impl<Act, A, R> EnvelopeProxy<Act> for ActionEnvelope<Act, A, R>
where
    Act: Actor,
    A: ActorAction<Act, Result = R>,
    R: Send + 'static,
{
    fn handle(self: Box<Self>, actor: &mut Act, ctx: &mut ActorContext<Act>) -> Action {
        let result: R = ActorAction::handle(self.action, actor, ctx);
        self.tx.send(result).ok();
        Action::Continue
    }
}

struct DiscardActionEnvelope<Act: Actor, A> {
    /// The action to execute
    action: A,
    /// Marker for storing the actor type
    _marker: PhantomData<Act>,
}

impl<Act, A> EnvelopeProxy<Act> for DiscardActionEnvelope<Act, A>
where
    Act: Actor,
    A: ActorAction<Act>,
{
    fn handle(self: Box<Self>, actor: &mut Act, ctx: &mut ActorContext<Act>) -> Action {
        ActorAction::handle(self.action, actor, ctx);
        Action::Continue
    }
}
