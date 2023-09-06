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
use std::{
    collections::HashMap,
    future::Future,
    hash::{BuildHasherDefault, Hasher},
    marker::PhantomData,
};
use tdf::DecodeError;

/// Error that can occur while handling a packet
#[derive(Debug)]
pub enum HandleError {
    /// There wasn't an available handler for the provided packet
    MissingHandler,
    /// Decoding error while reading the packet
    Decoding(DecodeError),
}

/// Type for handlers that include a request and response
pub struct HandlerRequest<Req, Res>(PhantomData<fn(Req) -> Res>);
/// Type for handlers that include a response but no request
pub struct HandlerOmitRequest<Res>(PhantomData<fn() -> Res>);

type HandleResult<'a> = Result<BoxFuture<'a, Packet>, HandleError>;

pub trait Handler<'a, Type>: Send + Sync + 'static {
    fn handle(&self, state: &'a SessionLink, packet: &'a Packet) -> HandleResult<'a>;
}

impl<'a, Fun, Fut, Req, Res> Handler<'a, HandlerRequest<Req, Res>> for Fun
where
    Fun: Fn(&'a SessionLink, Req) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Req: FromRequest,
    Res: IntoResponse,
{
    fn handle(&self, state: &'a SessionLink, packet: &'a Packet) -> HandleResult<'a> {
        let req = Req::from_request(packet).map_err(HandleError::Decoding)?;
        let future = self(state, req);
        Ok(Box::pin(async move {
            let res = future.await;
            res.into_response(packet)
        }))
    }
}

impl<'a, Fun, Fut, Res> Handler<'a, HandlerOmitRequest<Res>> for Fun
where
    Fun: Fn(&'a SessionLink) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Res: IntoResponse,
{
    fn handle(&self, state: &'a SessionLink, packet: &'a Packet) -> HandleResult<'a> {
        let future = self(state);
        Ok(Box::pin(async move {
            let res = future.await;
            res.into_response(packet)
        }))
    }
}

struct HandlerRoute<H, Format> {
    handler: H,
    _marker: PhantomData<fn(Format)>,
}

trait Route: Send + Sync {
    fn handle<'s>(&self, state: &'s SessionLink, packet: &'s Packet) -> HandleResult<'s>;
}

impl<H, Format> Route for HandlerRoute<H, Format>
where
    for<'a> H: Handler<'a, Format>,
    Format: 'static,
{
    fn handle<'s>(&self, state: &'s SessionLink, packet: &'s Packet) -> HandleResult<'s> {
        self.handler.handle(state, packet)
    }
}

pub struct Router {
    /// Map for looking up a route based on the component key
    routes: HashMap<ComponentKey, Box<dyn Route>, BuildHasherDefault<ComponentKeyHasher>>,
}

impl Router {
    pub fn new() -> Self {
        Self {
            routes: Default::default(),
        }
    }

    pub fn route<Format>(
        &mut self,
        component: u16,
        command: u16,
        route: impl for<'a> Handler<'a, Format>,
    ) where
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

    pub fn handle<'a>(&self, state: &'a SessionLink, packet: &'a Packet) -> HandleResult<'a> {
        self.routes
            .get(&component_key(
                packet.header.component,
                packet.header.command,
            ))
            .ok_or(HandleError::MissingHandler)?
            .handle(state, packet)
    }
}

/// "Hasher" used by the router map that just directly stores the integer value
/// from the component key as no hashing is required
#[derive(Default)]
pub struct ComponentKeyHasher(u32);

impl Hasher for ComponentKeyHasher {
    fn finish(&self) -> u64 {
        self.0 as u64
    }

    fn write(&mut self, _bytes: &[u8]) {
        panic!("Attempted to use component key hasher to hash bytes")
    }

    fn write_u32(&mut self, i: u32) {
        self.0 = i;
    }
}
