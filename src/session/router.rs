//! Router implementation for routing packet components to different functions
//! and automatically decoding the packet contents to the function type

use std::{collections::HashMap, future::Future, marker::PhantomData, pin::Pin};

use tdf::DecodeError;

use crate::utils::types::BoxFuture;

use super::{
    packet::{FromRequest, IntoResponse, Packet},
    SessionLink,
};

pub struct FormatA;
pub struct FormatB;

type HandleResult<'a> = Result<BoxFuture<'a, Packet>, HandleError>;

pub trait Handler<'a, Req, Res, Format>: Send + Sync + 'static {
    fn handle(&self, state: &'a mut SessionLink, packet: &'a Packet) -> HandleResult<'a>;
}

impl<'a, Fun, Fut, Req, Res> Handler<'a, Req, Res, FormatA> for Fun
where
    Fun: Fn(&'a mut SessionLink, Req) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Req: FromRequest,
    Res: IntoResponse,
{
    fn handle(&self, state: &'a mut SessionLink, packet: &'a Packet) -> HandleResult<'a> {
        let req = Req::from_request(packet).map_err(HandleError::Decoding)?;
        let future = self(state, req);
        Ok(Box::pin(async move {
            let res = future.await;
            res.into_response(packet)
        }))
    }
}

impl<'a, Fun, Fut, Res> Handler<'a, (), Res, FormatB> for Fun
where
    Fun: Fn(&'a mut SessionLink) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Res: IntoResponse,
{
    fn handle(&self, state: &'a mut SessionLink, packet: &'a Packet) -> HandleResult<'a> {
        let future = self(state);
        Ok(Box::pin(async move {
            let res = future.await;
            res.into_response(packet)
        }))
    }
}

trait Route: Send + Sync {
    fn handle<'s>(&self, state: &'s mut SessionLink, packet: &'s Packet) -> HandleResult<'s>;
}

struct HandlerRoute<H, Req, Res, Format> {
    handler: H,
    _marker: PhantomData<fn(Req, Format) -> Res>,
}

impl<H, Req, Res, Format> Route for HandlerRoute<H, Req, Res, Format>
where
    for<'a> H: Handler<'a, Req, Res, Format>,
    Req: FromRequest,
    Res: IntoResponse,
    Format: 'static,
{
    fn handle<'s>(&self, state: &'s mut SessionLink, packet: &'s Packet) -> HandleResult<'s> {
        self.handler.handle(state, packet)
    }
}

#[derive(Default)]
pub struct Router {
    routes: HashMap<(u16, u16), Box<dyn Route>>,
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn route<Req, Res, Format>(
        &mut self,
        component: u16,
        command: u16,
        route: impl for<'a> Handler<'a, Req, Res, Format>,
    ) where
        Req: FromRequest,
        Res: IntoResponse,
        Format: 'static,
    {
        self.routes.insert(
            (component, command),
            Box::new(HandlerRoute {
                handler: route,
                _marker: PhantomData,
            }),
        );
    }

    pub fn handle<'a>(&self, state: &'a mut SessionLink, packet: &'a Packet) -> HandleResult<'a> {
        let route = self
            .routes
            .get(&(packet.header.command, packet.header.command))
            .ok_or(HandleError::MissingHandler)?;
        route.handle(state, packet)
    }
}

/// Error that can occur while handling a packet
#[derive(Debug)]
pub enum HandleError {
    /// There wasn't an available handler for the provided packet
    MissingHandler,
    /// Decoding error while reading the packet
    Decoding(DecodeError),
}
