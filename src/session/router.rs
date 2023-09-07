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

/// Wrapper around [Handler] that stores the required associated
/// generic types allowing it to have its typed erased using [ErasedHandler]
struct HandlerRoute<H, Format> {
    /// The wrapped handler
    handler: H,
    /// The associated type info
    _marker: PhantomData<fn(Format)>,
}

/// Wrapper around [Handler] that erasings the associated generic types
/// so that it can be stored within the [Router]
trait ErasedHandler: Send + Sync {
    fn handle<'f, 'a>(&'f self, req: PacketRequest<'a>) -> BoxFuture<'a, Packet>
    where
        'f: 'a;
}

impl<H, Format> ErasedHandler for HandlerRoute<H, Format>
where
    for<'a> H: Handler<'a, Format>,
    Format: 'static,
{
    #[inline]
    fn handle<'f, 'a>(&'f self, req: PacketRequest<'a>) -> BoxFuture<'a, Packet>
    where
        'f: 'a,
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
    routes: HashMap<ComponentKey, Box<dyn ErasedHandler>, BuildHasherDefault<ComponentKeyHasher>>,
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

pub trait IntoPacketResponse: 'static {
    fn into_response(self, req: &Packet) -> Packet;
}

impl IntoPacketResponse for () {
    fn into_response(self, req: &Packet) -> Packet {
        Packet::response_empty(req)
    }
}

impl IntoPacketResponse for Infallible {
    fn into_response(self, _: &Packet) -> Packet {
        // Infallible can never be constructed so this can never happen
        unreachable!()
    }
}

impl IntoPacketResponse for Packet {
    fn into_response(self, _req: &Packet) -> Packet {
        self
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
        Packet::new_response(req, self.0)
    }
}

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

// Macro for expanding a macro for every tuple variant
#[rustfmt::skip]
macro_rules! all_the_tuples {
    ($name:ident) => {
        $name!([]);
        $name!([T1]);
        $name!([T1, T2]);
        $name!([T1, T2, T3]);
        $name!([T1, T2, T3, T4]);
        $name!([T1, T2, T3, T4, T5]);
        $name!([T1, T2, T3, T4, T5, T6]);
        $name!([T1, T2, T3, T4, T5, T6, T7]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15]);
    };
}

// Macro for implementing a handler for a tuple of arguments
macro_rules! impl_handler {
    (
        [$($ty:ident),*]
    ) => {

        #[allow(non_snake_case, unused_mut)]
        impl<'a, Fun, Fut, $($ty,)* Res> Handler<'a, HandlerRequest<($($ty,)*), Res>> for Fun
        where
            Fun: Fn($($ty),*) -> Fut + Send + Sync + 'static,
            Fut: Future<Output = Res> + Send + 'a,
            $( $ty: FromPacketRequest + Send, )*
            Res: IntoPacketResponse,
        {
            fn handle<'f>(&'f self, req: PacketRequest<'a>) -> BoxFuture<'a, Packet>
            where
                'f: 'a,
            {
                Box::pin(async move {
                    let req = req;
                    $(

                        let $ty = match $ty::from_packet_request(&req).await {
                            Ok(value) => value,
                            Err(rejection) => return rejection.into_response(req.packet),
                        };
                    )*

                    let res = self($($ty),* ).await;
                    res.into_response(req.packet)
                })
            }
        }
    };
}

// Implement a handler for every tuple
all_the_tuples!(impl_handler);
