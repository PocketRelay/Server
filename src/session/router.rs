//! Router implementation for routing packet components to different functions
//! and automatically decoding the packet contents to the function type

use super::{
    models::errors::BlazeError,
    packet::{Packet, PacketHeader},
    SessionLink,
};
use crate::{
    session::models::errors::GlobalError,
    utils::{
        components::{component_key, ComponentKey},
        types::BoxFuture,
    },
};
use bytes::Bytes;
use log::error;
use std::{
    collections::HashMap,
    convert::Infallible,
    future::{ready, Future},
    hash::{BuildHasherDefault, Hasher},
    marker::PhantomData,
};
use tdf::{serialize_vec, TdfDeserialize, TdfDeserializer, TdfSerialize};

/// Type for handlers that include a request and response
pub struct HandlerRequest<Req, Res>(PhantomData<fn(Req) -> Res>);

pub trait Handler<'a, Args>: Send + Sync + 'static {
    fn handle<'f>(&'f self, req: PacketRequest<'a>) -> BoxFuture<'a, Packet>
    where
        'f: 'a;
}

impl<'a, Fun, Fut, A, B, Res> Handler<'a, HandlerRequest<(A, B), Res>> for Fun
where
    Fun: Fn(A, B) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    A: FromPacketRequest + Send,
    B: FromPacketRequest + Send,
    Res: IntoPacketResponse,
{
    fn handle<'f>(&'f self, req: PacketRequest<'a>) -> BoxFuture<'a, Packet>
    where
        'f: 'a,
    {
        Box::pin(async move {
            let req = req;
            let a = match A::from_packet_request(&req).await {
                Ok(value) => value,
                Err(error) => return error.into_response(req.packet),
            };
            let b = match B::from_packet_request(&req).await {
                Ok(value) => value,
                Err(error) => return error.into_response(req.packet),
            };
            let res = self(a, b).await;
            res.into_response(req.packet)
        })
    }
}
impl<'a, Fun, Fut, A, Res> Handler<'a, HandlerRequest<A, Res>> for Fun
where
    Fun: Fn(A) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    A: FromPacketRequest + Send,
    Res: IntoPacketResponse,
{
    fn handle<'f>(&'f self, req: PacketRequest<'a>) -> BoxFuture<'a, Packet>
    where
        'f: 'a,
    {
        Box::pin(async move {
            let req = req;
            let a = match A::from_packet_request(&req).await {
                Ok(value) => value,
                Err(error) => return error.into_response(req.packet),
            };

            let res = self(a).await;
            res.into_response(req.packet)
        })
    }
}

pub struct Nothing;

impl<'a, Fun, Fut, Res> Handler<'a, HandlerRequest<Nothing, Res>> for Fun
where
    Fun: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Res: IntoPacketResponse,
{
    fn handle<'f>(&'f self, req: PacketRequest<'a>) -> BoxFuture<'a, Packet>
    where
        'f: 'a,
    {
        Box::pin(async move {
            let res = self().await;
            res.into_response(req.packet)
        })
    }
}

struct HandlerRoute<H, Format> {
    handler: H,
    _marker: PhantomData<fn(Format)>,
}

trait Route: Send + Sync {
    fn handle<'f, 's>(&'f self, req: PacketRequest<'s>) -> BoxFuture<'s, Packet>
    where
        'f: 's;
}

impl<H, Format> Route for HandlerRoute<H, Format>
where
    for<'a> H: Handler<'a, Format>,
    Format: 'static,
{
    fn handle<'f, 's>(&'f self, req: PacketRequest<'s>) -> BoxFuture<'s, Packet>
    where
        'f: 's,
    {
        self.handler.handle(req)
    }
}

pub struct PacketRequest<'a> {
    pub state: &'a SessionLink,
    pub packet: &'a Packet,
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

    pub fn handle<'r, 'a>(
        &'r self,
        state: &'a SessionLink,
        packet: &'a Packet,
    ) -> Option<BoxFuture<'a, Packet>>
    where
        'r: 'a,
    {
        Some(
            self.routes
                .get(&component_key(
                    packet.header.component,
                    packet.header.command,
                ))?
                .handle(PacketRequest { state, packet }),
        )
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

pub trait FromPacketRequest: Sized {
    type Rejection: IntoPacketResponse;

    fn from_packet_request<'a>(
        req: &PacketRequest<'a>,
    ) -> BoxFuture<'a, Result<Self, Self::Rejection>>
    where
        Self: 'a;
}

impl FromPacketRequest for SessionLink {
    type Rejection = Infallible;

    fn from_packet_request<'a>(
        req: &PacketRequest<'a>,
    ) -> BoxFuture<'a, Result<Self, Self::Rejection>>
    where
        Self: 'a,
    {
        let state = req.state;
        Box::pin(ready(Ok(state.clone())))
    }
}

/// Wrapper for providing deserialization [FromPacketRequest] and
/// serialization [IntoPacketResponse] for TDF contents
pub struct Blaze<V>(pub V);

/// Wrapper for providing deserialization [FromPacketRequest] and
/// serialization [IntoPacketResponse] for TDF contents
///
/// Stores the packet header so that it can be used for generating
/// responses
pub struct BlazeWithHeader<V> {
    pub req: V,
    pub header: PacketHeader,
}

/// [Blaze] tdf type for contents that have already been
/// serialized ahead of time
pub struct RawBlaze(Bytes);

impl<T> From<T> for RawBlaze
where
    T: TdfSerialize,
{
    fn from(value: T) -> Self {
        let bytes = serialize_vec(&value);
        let bytes = Bytes::from(bytes);
        RawBlaze(bytes)
    }
}

impl<V> FromPacketRequest for Blaze<V>
where
    for<'a> V: TdfDeserialize<'a> + Send + 'a,
{
    type Rejection = BlazeError;

    fn from_packet_request<'a>(
        req: &PacketRequest<'a>,
    ) -> BoxFuture<'a, Result<Self, Self::Rejection>>
    where
        Self: 'a,
    {
        let result = match req.packet.deserialize::<'a, V>() {
            Ok(value) => Ok(Blaze(value)),
            Err(err) => {
                error!("Error while decoding packet: {:?}", err);
                Err(GlobalError::System.into())
            }
        };

        Box::pin(ready(result))
    }
}

impl<V> BlazeWithHeader<V>
where
    for<'a> V: TdfDeserialize<'a> + Send + 'a,
{
    pub fn response<E>(&self, res: E) -> Packet
    where
        E: TdfSerialize,
    {
        Packet {
            header: self.header.response(),
            contents: Bytes::from(serialize_vec(&res)),
        }
    }
}

impl<V> FromPacketRequest for BlazeWithHeader<V>
where
    for<'a> V: TdfDeserialize<'a> + Send + 'a,
{
    type Rejection = BlazeError;

    fn from_packet_request<'a>(
        req: &PacketRequest<'a>,
    ) -> BoxFuture<'a, Result<Self, Self::Rejection>>
    where
        Self: 'a,
    {
        let mut r = TdfDeserializer::new(&req.packet.contents);

        Box::pin(ready(
            V::deserialize(&mut r)
                .map(|value| BlazeWithHeader {
                    req: value,
                    header: req.packet.header,
                })
                .map_err(|err| {
                    error!("Error while decoding packet: {:?}", err);
                    GlobalError::System.into()
                }),
        ))
    }
}

impl IntoPacketResponse for Packet {
    /// Simply provide the already compute response
    fn into_response(self, _req: &Packet) -> Packet {
        self
    }
}

/// Trait for a type that can be converted into a packet
/// response using the header from the request packet
pub trait IntoPacketResponse: 'static {
    /// Into packet conversion
    fn into_response(self, req: &Packet) -> Packet;
}

/// Into response imeplementation for encodable responses
/// which just calls res.respond
impl IntoPacketResponse for () {
    fn into_response(self, req: &Packet) -> Packet {
        Packet::response_empty(req)
    }
}

impl IntoPacketResponse for Infallible {
    fn into_response(self, _: &Packet) -> Packet {
        unreachable!("Request should **never** fail")
    }
}

impl<V> IntoPacketResponse for Blaze<V>
where
    V: TdfSerialize + 'static,
{
    fn into_response(self, req: &Packet) -> Packet {
        Packet::response(req, self.0)
    }
}

impl IntoPacketResponse for RawBlaze {
    fn into_response(self, req: &Packet) -> Packet {
        Packet {
            header: req.header.response(),
            contents: self.0,
        }
    }
}

/// Into response imeplementation for encodable responses
/// which just calls res.respond
impl<A, B> IntoPacketResponse for Result<A, B>
where
    A: IntoPacketResponse,
    B: IntoPacketResponse,
{
    fn into_response(self, req: &Packet) -> Packet {
        match self {
            Ok(value) => value.into_response(req),
            Err(value) => value.into_response(req),
        }
    }
}
/// Into response imeplementation for encodable responses
/// which just calls res.respond
impl<A> IntoPacketResponse for Option<A>
where
    A: IntoPacketResponse,
{
    fn into_response(self, req: &Packet) -> Packet {
        match self {
            Some(value) => value.into_response(req),
            None => Packet::response_empty(req),
        }
    }
}
