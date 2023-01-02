use blaze_pk::{
    error::DecodeResult,
    packet::{FromRequest, IntoResponse, Packet, PacketComponents},
};
use pin_project::pin_project;
use std::{collections::HashMap, future::Future, marker::PhantomData, pin::Pin, task::Poll};

use crate::blaze::components::Components;

use super::session::Session;

type RouteFuture = DecodeResult<Pin<Box<dyn Future<Output = Packet> + Send>>>;

pub struct Router {
    routes: HashMap<Components, Box<dyn Route>>,
}

impl Router {
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    pub fn route<F, R>(&mut self, component: Components, route: F) -> &mut Self
    where
        F: IntoRoute<R>,
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
        let component = Components::from_header(&packet.header);
        let route = match self.routes.get(&component) {
            Some(value) => value,
            None => return Ok(packet.respond_empty()),
        };
        Ok(route.handle(Pin::new(state), packet)?.await)
    }
}

trait Route: Send + Sync {
    fn handle(&self, session: Pin<&mut Session>, packet: Packet) -> RouteFuture;
}

struct Empty;

trait IntoRoute<Args> {
    fn into_route(self) -> Box<dyn Route>;
}

struct StatelessRoute<F, Fut, Res, Req> {
    inner: F,
    _marker: PhantomData<fn() -> (Fut, Req, Res)>,
}

impl<F, Fut, Res, Req> Route for StatelessRoute<F, Fut, Res, Req>
where
    F: Fn(Req) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'static,
    Req: FromRequest + 'static,
    Res: IntoResponse + 'static,
{
    fn handle(&self, _session: Pin<&mut Session>, packet: Packet) -> RouteFuture {
        let req: Req = FromRequest::from_request(&packet)?;
        let fut: Fut = (self.inner)(req);
        Ok(Box::pin(Handle {
            inner: fut,
            packet,
            _marker: PhantomData,
        }))
    }
}

impl<F, Fut, Res, Req> IntoRoute<(Empty, Req)> for F
where
    F: Fn(Req) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'static,
    Req: FromRequest + 'static,
    Res: IntoResponse + 'static,
{
    fn into_route(self) -> Box<dyn Route> {
        Box::new(StatelessRoute {
            inner: self,
            _marker: PhantomData as PhantomData<fn() -> (Fut, Req, Res)>,
        })
    }
}

impl<F, Fut, Res> Route for StatelessRoute<F, Fut, Res, Empty>
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'static,
    Res: IntoResponse + 'static,
{
    fn handle(&self, _session: Pin<&mut Session>, packet: Packet) -> RouteFuture {
        let fut: Fut = (self.inner)();
        Ok(Box::pin(Handle {
            inner: fut,
            packet,
            _marker: PhantomData,
        }))
    }
}

impl<F, Fut, Res> IntoRoute<(Empty, Empty)> for F
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'static,
    Res: IntoResponse + 'static,
{
    fn into_route(self) -> Box<dyn Route> {
        Box::new(StatelessRoute {
            inner: self,
            _marker: PhantomData as PhantomData<fn() -> (Fut, Empty, Res)>,
        })
    }
}

struct StateRoute<F, Fut, Res, Req> {
    inner: F,
    _marker: PhantomData<fn() -> (Fut, Req, Res)>,
}

impl<F, Fut, Res, Req> Route for StateRoute<F, Fut, Res, Req>
where
    F: for<'a> Fn(&'a mut Session, Req) -> Fut + Send + Sync,
    Fut: Future<Output = Res> + Send,
    Req: FromRequest + 'static,
    Res: IntoResponse + 'static,
{
    fn handle(&self, session: Pin<&mut Session>, packet: Packet) -> RouteFuture {
        let req: Req = FromRequest::from_request(&packet)?;
        let fut: Fut = (self.inner)(session.get_mut(), req);
        Ok(Box::pin(Handle {
            inner: fut,
            packet,
            _marker: PhantomData,
        }))
    }
}

impl<F, Fut, Res, Req> IntoRoute<(Session, Req)> for F
where
    F: for<'a> Fn(&'a mut Session, Req) -> Fut + Send + Sync,
    Fut: Future<Output = Res> + Send,
    Req: FromRequest + 'static,
    Res: IntoResponse + 'static,
{
    fn into_route(self) -> Box<dyn Route> {
        Box::new(StateRoute {
            inner: self,
            _marker: PhantomData as PhantomData<fn() -> (Fut, Req, Res)>,
        })
    }
}

impl<F, Fut, Res> Route for StateRoute<F, Fut, Res, Empty>
where
    F: for<'a> Fn(&'a mut Session) -> Fut + Send + Sync,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
{
    fn handle(&self, session: Pin<&mut Session>, packet: Packet) -> RouteFuture {
        let fut: Fut = (self.inner)(session.get_mut());
        Ok(Box::pin(Handle {
            inner: fut,
            packet,
            _marker: PhantomData,
        }))
    }
}

impl<F, Fut, Res> IntoRoute<Session> for F
where
    F: for<'a> Fn(&'a mut Session) -> Fut + Send + Sync,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
{
    fn into_route(self) -> Box<dyn Route> where {
        Box::new(StateRoute {
            inner: self,
            _marker: PhantomData as PhantomData<fn() -> (Fut, Empty, Res)>,
        })
    }
}

#[pin_project]
struct Handle<'a, Fut, Res> {
    #[pin]
    inner: Fut,
    packet: Packet,
    _marker: PhantomData<fn(&'a mut Session) -> Res>,
}

impl<'a, Fut, Res> Future for Handle<'a, Fut, Res>
where
    Fut: Future<Output = Res> + Send + 'a,
    Res: IntoResponse,
{
    type Output = Packet;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let it = self.project();
        let res = match it.inner.poll(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(res) => res,
        };
        let response = res.into_response(&it.packet);
        Poll::Ready(response)
    }
}
