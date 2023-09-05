//! Router implementation for routing packet components to different functions
//! and automatically decoding the packet contents to the function type

use super::{
    packet::{FromRequest, IntoResponse, Packet},
    SessionLink,
};
use crate::utils::{
    components::{component_key, ComponentKey},
    types::BoxFuture,
};
use std::{collections::HashMap, future::Future, marker::PhantomData};
use tdf::DecodeError;

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

struct HandlerRoute<H, Req, Res, Format> {
    handler: H,
    _marker: PhantomData<fn(Req, Format) -> Res>,
}

trait Route: Send + Sync {
    fn handle<'s>(&self, state: &'s mut SessionLink, packet: &'s Packet) -> HandleResult<'s>;
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

pub struct Router {
    routes: HashMap<ComponentKey, Box<dyn Route>>,
}

impl Router {
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
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
            component_key(component, command),
            Box::new(HandlerRoute {
                handler: route,
                _marker: PhantomData,
            }),
        );
    }

    pub fn handle<'a>(&self, state: &'a mut SessionLink, packet: &'a Packet) -> HandleResult<'a> {
        self.routes
            .get(&component_key(
                packet.header.component,
                packet.header.command,
            ))
            .ok_or(HandleError::MissingHandler)?
            .handle(state, packet)
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
