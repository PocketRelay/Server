use std::{collections::HashMap, future::Future, marker::PhantomData, pin::Pin};

use blaze_pk::{
    codec::Decodable,
    error::DecodeResult,
    packet::{IntoResponse, Packet, PacketComponents},
};

use super::session::Session;
use crate::blaze::components::Components as C;

pub struct Router {
    routes: HashMap<C, Box<dyn Route>>,
}

impl Router {
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    /// Adds a new route that doesn't require state to be provided
    ///
    /// `component` The route component
    /// `route`     The route function
    pub fn route<R, T>(&mut self, component: C, route: R) -> &mut Self
    where
        R: IntoRoute<T>,
    {
        self.routes.insert(component, route.into_route());
        self
    }

    /// Handles the routing for the provided packet with
    /// the provided handle state. Will return the response
    /// packet or a decoding error for failed req decodes.
    /// Will return an empty packet for routes that are not
    /// registered
    ///
    /// `state`  The additional handle state
    /// `packet` The packet to handle routing
    pub async fn handle(&self, state: &mut Session, packet: Packet) -> DecodeResult<Packet> {
        let component = C::from_header(&packet.header);
        let route = match self.routes.get(&component) {
            Some(value) => value,
            None => return Ok(packet.respond_empty()),
        };
        route.handle(state, packet).await
    }
}

type RouteFuture = Pin<Box<dyn Future<Output = DecodeResult<Packet>> + Send>>;

trait IntoRoute<T> {
    fn into_route(self) -> Box<dyn Route>;
}

/// Route implementation used for handling requests
trait Route: Send + Sync {
    /// Handles the routing for this route using the provided
    /// state and processing the provided packet
    ///
    /// `state`  The additional state
    /// `packet` The packet to handle
    fn handle(&self, state: &mut Session, packet: Packet) -> RouteFuture;
}

/// Function based route implementation which wraps a function
/// that is used to handle a request and return a response
struct FnRoute<F, T> {
    /// The inner route function handle
    inner: F,
    /// Phantom data for storing the associated types
    _marker: PhantomData<fn() -> T>,
}

impl<F, T> Route for FnRoute<F, T>
where
    F: FnHandle<T>,
{
    fn handle(&self, state: &mut Session, packet: Packet) -> RouteFuture {
        let inner = self.inner.clone();
        Box::pin(inner.handle(state, packet))
    }
}

trait FnHandle<T>: Clone + Send + Sync + Sized + 'static {
    fn handle(self, state: &mut Session, packet: Packet) -> RouteFuture;
}

impl<H, T> IntoRoute<T> for H
where
    H: FnHandle<T>,
{
    fn into_route(self) -> Box<dyn Route> {
        Box::new(FnRoute {
            inner: self,
            _marker: PhantomData as PhantomData<fn() -> T>,
        })
    }
}

async fn test(session: &mut Session) -> () {
    ()
}

fn test1() {
    let route = test.into_route();
}

struct Nil;

/// Handle implementation for functions that take the session state
/// argument as well as a request argument
///
/// ```
/// async fn test_route(state:  &mut Session) -> Res {
///
/// }
/// ```
impl<F, Res, Fut> FnHandle<(Session, Nil)> for F
where
    F: FnOnce(&mut Session) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'static,
    Res: IntoResponse + 'static,
{
    fn handle(self, state: &mut Session, packet: Packet) -> RouteFuture {
        Box::pin(async move {
            let res: Res = self(state).await;
            Ok(res.into_response(packet))
        })
    }
}

/// Handle implementation for functions that take the session state
/// argument as well as a request argument
///
/// ```
/// async fn test_route(state:  &mut Session, req: Req) -> Res {
///
/// }
/// ```
impl<F, Req, Res, Fut> FnHandle<(Session, Req)> for F
where
    F: FnOnce(&mut Session, Req) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send,
    Req: Decodable + Send + 'static,
    Res: IntoResponse + 'static,
{
    fn handle(self, state: &mut Session, packet: Packet) -> RouteFuture {
        Box::pin(async move {
            let req: Req = packet.decode()?;
            let res: Res = self(state, req).await;
            Ok(res.into_response(packet))
        })
    }
}

/// Handle implementation for functions that take the session state
/// argument as well as a request argument
///
/// ```
/// async fn test_route(req: Req) -> Res {
///
/// }
/// ```
impl<F, Req, Res, Fut> FnHandle<(Nil, Req)> for F
where
    F: FnOnce(Req) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send,
    Req: Decodable + Send + 'static,
    Res: IntoResponse + 'static,
{
    fn handle(self, _state: &mut Session, packet: Packet) -> RouteFuture {
        Box::pin(async move {
            let req: Req = packet.decode()?;
            let res: Res = self(req).await;
            Ok(res.into_response(packet))
        })
    }
}

/// Handle implementation for functions that take the session state
/// argument as well as a request argument
///
/// ```
/// async fn test_route() -> Res {
///
/// }
/// ```
impl<F, Res, Fut> FnHandle<(Nil, Nil)> for F
where
    F: FnOnce() -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse + 'static,
{
    fn handle(self, _state: &mut Session, packet: Packet) -> RouteFuture {
        Box::pin(async move {
            let res: Res = self().await;
            Ok(res.into_response(packet))
        })
    }
}
