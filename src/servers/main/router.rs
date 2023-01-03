use blaze_pk::{
    error::DecodeResult,
    packet::{FromRequest, IntoResponse, Packet, PacketComponents},
};

use std::{collections::HashMap, future::Future, marker::PhantomData, pin::Pin};

use crate::blaze::components::Components;

use super::session::Session;

type RouteFuture<'a> = Pin<Box<dyn Future<Output = DecodeResult<Packet>> + Send + 'a>>;

pub struct Router {
    routes: HashMap<Components, Box<dyn for<'a> Route<'a>>>,
}

impl Router {
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    pub fn route<R, I>(&mut self, component: Components, route: R)
    where
        R: IntoRoute<I>,
    {
        self.routes.insert(component, route.into_route());
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
        route.handle(Pin::new(state), packet).await
    }
}

pub trait Route<'a>: Send + Sync {
    // type Future: Future<Output = Packet> + Send;

    fn handle(&self, session: Pin<&'a mut Session>, packet: Packet) -> RouteFuture<'a>;
}

pub struct Empty;
pub struct Nil;

impl FromRequest for Empty {
    fn from_request(_req: &Packet) -> DecodeResult<Self> {
        Ok(Self)
    }
}
pub trait IntoRoute<Args> {
    fn into_route(self) -> Box<dyn for<'a> Route<'a>>;
}

pub struct StatelessRoute<F, Fut, Res, Req> {
    inner: F,
    _marker: PhantomData<fn() -> (Fut, Req, Res)>,
}

impl<'a, F, Fut, Res, Req> Route<'a> for StatelessRoute<F, Fut, Res, Req>
where
    F: FnOnce(Req) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Req: FromRequest + 'static + Send,
    Res: IntoResponse + 'static,
{
    fn handle(&self, session: Pin<&'a mut Session>, packet: Packet) -> RouteFuture<'a> {
        let inner = self.inner.clone();
        Box::pin(map_future(session, packet, move |_, req| inner(req)))
    }
}

impl<'b, F, Fut, Res, Req> IntoRoute<StatelessRoute<F, Fut, Res, Req>> for F
where
    F: FnOnce(Req) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'b,
    Req: FromRequest + 'static + Send,
    Res: IntoResponse + 'static,
{
    fn into_route(self) -> Box<dyn for<'a> Route<'a>> {
        Box::new(StatelessRoute {
            inner: self,
            _marker: PhantomData,
        } as StatelessRoute<F, Fut, Res, Req>)
    }
}

impl<'a, F, Fut, Res> Route<'a> for StatelessRoute<F, Fut, Res, Nil>
where
    F: FnOnce() -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Res: IntoResponse + 'static,
{
    fn handle(&self, session: Pin<&'a mut Session>, packet: Packet) -> RouteFuture<'a> {
        let inner = self.inner.clone();
        Box::pin(map_future(session, packet, move |_, _: Empty| inner()))
    }
}

impl<'b, F, Fut, Res> IntoRoute<StatelessRoute<F, Fut, Res, Nil>> for F
where
    F: FnOnce() -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'b,
    Res: IntoResponse + 'static,
{
    fn into_route(self) -> Box<dyn for<'a> Route<'a>> {
        Box::new(StatelessRoute {
            inner: self,
            _marker: PhantomData,
        } as StatelessRoute<F, Fut, Res, Nil>)
    }
}

pub struct StateRoute<F, Fut, Res, Req> {
    inner: F,
    _marker: PhantomData<fn() -> (Fut, Req, Res)>,
}

impl<'a, F, Fut, Res, Req> Route<'a> for StateRoute<F, Fut, Res, Req>
where
    F: FnOnce(&'a mut Session, Req) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Req: FromRequest + 'static + Send,
    Res: IntoResponse + 'static,
{
    fn handle(&self, session: Pin<&'a mut Session>, packet: Packet) -> RouteFuture<'a> {
        let inner = self.inner.clone();

        Box::pin(map_future(session, packet, move |session, req| {
            inner(session, req)
        }))
    }
}

impl<F, Fut, Res, Req> IntoRoute<StateRoute<F, Fut, Res, Req>> for F
where
    for<'a> F: Fn(&'a mut Session, Req) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send,
    Req: FromRequest + 'static + Send,
    Res: IntoResponse + 'static,
{
    fn into_route(self) -> Box<dyn for<'a> Route<'a>> {
        Box::new(StateRoute {
            inner: self,
            _marker: PhantomData,
        } as StateRoute<F, Fut, Res, Req>)
    }
}

impl<'a, F, Fut, Res> Route<'a> for StateRoute<F, Fut, Res, Nil>
where
    F: Fn(&'a mut Session) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Res: IntoResponse + 'static,
{
    fn handle(&self, session: Pin<&'a mut Session>, packet: Packet) -> RouteFuture<'a> {
        let inner = self.inner.clone();
        Box::pin(map_future(session, packet, move |session, _: Empty| {
            inner(session)
        }))
    }
}

impl<F, Fut, Res> IntoRoute<StateRoute<F, Fut, Res, Nil>> for F
where
    for<'a> F: Fn(&'a mut Session) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse + 'static,
{
    fn into_route(self) -> Box<dyn for<'a> Route<'a>> {
        Box::new(StateRoute {
            inner: self,
            _marker: PhantomData,
        } as StateRoute<F, Fut, Res, Nil>)
    }
}

pub async fn map_future<'a: 'b, 'b, I, Fut, Req, Res>(
    session: Pin<&'a mut Session>,
    packet: Packet,
    inner: I,
) -> DecodeResult<Packet>
where
    I: FnOnce(&'b mut Session, Req) -> Fut,
    Fut: Future<Output = Res> + Send + 'b,
    Req: FromRequest,
    Res: IntoResponse,
{
    let req: Req = FromRequest::from_request(&packet)?;
    let res: Res = inner(session.get_mut(), req).await;
    Ok(res.into_response(&packet))
}
