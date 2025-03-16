//! Router implementation for routing packet components to different functions
//! and automatically decoding the packet contents to the function type

use super::{SessionLink, models::errors::BlazeError, packet::Packet};
use crate::{
    database::entities::{Player, PlayerData},
    services::game::{GamePlayer, GamePlayerPlayerDataSnapshot},
    session::models::errors::GlobalError,
    utils::{
        components::{ComponentKey, component_key},
        hashing::IntHashMap,
    },
};
use bytes::Bytes;
use futures_util::future::BoxFuture;
use log::{debug, error};
use sea_orm::DatabaseConnection;
use std::{
    any::{Any, TypeId},
    convert::Infallible,
    future::ready,
    marker::PhantomData,
    sync::Arc,
};
use tdf::{TdfDeserialize, TdfSerialize, serialize_vec};

pub trait Handler<Args, Res>: Send + Sync + 'static {
    fn handle(&self, req: PacketRequest) -> BoxFuture<'_, Packet>;
}

/// Wrapper around [Handler] that stores the required associated
/// generic types allowing it to have its typed erased using [ErasedHandler]
struct HandlerRoute<H, Args, Res> {
    /// The wrapped handler
    handler: H,
    /// The associated type info
    _marker: PhantomData<fn(Args) -> Res>,
}

/// Wrapper around [Handler] that erases the associated generic types
/// so that it can be stored within the [Router]
trait ErasedHandler: Send + Sync {
    fn handle(&self, req: PacketRequest) -> BoxFuture<'_, Packet>;
}

/// Erased handler implementation for all [Handler] implementations using [HandlerRoute]
impl<H, Args, Res> ErasedHandler for HandlerRoute<H, Args, Res>
where
    H: Handler<Args, Res>,
    Args: 'static,
    Res: 'static,
{
    #[inline]
    fn handle(&self, req: PacketRequest) -> BoxFuture<'_, Packet> {
        self.handler.handle(req)
    }
}

/// Packet request to be handled, includes the packet itself
/// along with the session link the packet is being handled
/// by and the extensions map of provided extensions
pub struct PacketRequest {
    pub state: SessionLink,
    pub packet: Packet,
    pub extensions: Extensions,
}

#[derive(Clone)]
pub struct Extensions {
    inner: Arc<AnyMap>,
}

impl Extensions {
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.inner
            .get(&TypeId::of::<T>())
            .and_then(|boxed| (&**boxed as &(dyn Any + 'static)).downcast_ref())
    }
}

type AnyMap = IntHashMap<TypeId, Box<dyn Any + Send + Sync>>;
type RouteMap = IntHashMap<ComponentKey, Box<dyn ErasedHandler>>;

pub struct BlazeRouterBuilder {
    /// Map for looking up a route based on the component key
    routes: RouteMap,
    extensions: AnyMap,
}

impl BlazeRouterBuilder {
    pub fn new() -> Self {
        Self {
            routes: Default::default(),
            extensions: Default::default(),
        }
    }

    pub fn add_extension<T: Send + Sync + 'static>(&mut self, val: T) -> Option<T> {
        self.extensions
            .insert(TypeId::of::<T>(), Box::new(val))
            .and_then(|boxed| {
                (boxed as Box<dyn Any + 'static>)
                    .downcast()
                    .ok()
                    .map(|boxed| *boxed)
            })
    }

    pub fn extension<T: Send + Sync + 'static>(mut self, val: T) -> Self {
        self.add_extension(val);
        self
    }

    pub fn route<Args, Res>(&mut self, component: u16, command: u16, route: impl Handler<Args, Res>)
    where
        Args: 'static,
        Res: 'static,
    {
        self.routes.insert(
            component_key(component, command),
            Box::new(HandlerRoute {
                handler: route,
                _marker: PhantomData,
            }),
        );
    }

    pub fn build(self) -> Arc<BlazeRouter> {
        Arc::new(BlazeRouter {
            routes: self.routes,
            extensions: Extensions {
                inner: Arc::new(self.extensions),
            },
        })
    }
}

pub struct BlazeRouter {
    /// Map for looking up a route based on the component key
    routes: RouteMap,
    pub extensions: Extensions,
}

impl BlazeRouter {
    pub fn handle(&self, state: SessionLink, packet: Packet) -> BoxFuture<'_, Packet> {
        match self
            .routes
            .get(&component_key(packet.frame.component, packet.frame.command))
        {
            Some(route) => route.handle(PacketRequest {
                state,
                packet,
                extensions: self.extensions.clone(),
            }),
            // Respond with a default empty packet
            None => {
                debug!(
                    "Missing packet handler for {:#06x}->{:#06x}",
                    packet.frame.component, packet.frame.command
                );
                Box::pin(ready(Packet::response_empty(&packet)))
            }
        }
    }
}

pub trait FromPacketRequest: Sized {
    type Rejection: IntoPacketResponse;

    fn from_packet_request<'a>(
        req: &'a mut PacketRequest,
    ) -> BoxFuture<'a, Result<Self, Self::Rejection>>
    where
        Self: 'a;
}

/// Wrapper for providing deserialization [FromPacketRequest] and
/// serialization [IntoPacketResponse] for TDF contents
pub struct Blaze<V>(pub V);

/// [Blaze] tdf type for contents that have already been
/// serialized ahead of time
pub struct RawBlaze(Bytes);

/// Extracts the session authenticated player if one is present,
/// responds with [GlobalError::AuthenticationRequired] if there is none
pub struct SessionAuth(pub Arc<Player>);

pub struct Extension<T>(pub T);

impl<T> FromPacketRequest for Extension<T>
where
    T: Clone + Send + Sync + 'static,
{
    type Rejection = BlazeError;

    fn from_packet_request<'a>(
        req: &'a mut PacketRequest,
    ) -> BoxFuture<'a, Result<Self, Self::Rejection>>
    where
        Self: 'a,
    {
        Box::pin(ready(
            req.extensions
                .get()
                .ok_or_else(|| {
                    error!(
                        "Attempted to extract missing extension {}",
                        std::any::type_name::<T>()
                    );
                    GlobalError::System.into()
                })
                .cloned()
                .map(Extension),
        ))
    }
}

impl FromPacketRequest for GamePlayer {
    type Rejection = BlazeError;

    fn from_packet_request<'a>(
        req: &'a mut PacketRequest,
    ) -> BoxFuture<'a, Result<Self, Self::Rejection>>
    where
        Self: 'a,
    {
        Box::pin(async move {
            let db = req
                .extensions
                .get::<DatabaseConnection>()
                .ok_or(GlobalError::System)?;

            let player = req
                .state
                .data
                .get_player()
                .ok_or(GlobalError::AuthenticationRequired)?;

            let player_data: Vec<(String, String)> = PlayerData::all(db, player.id)
                .await?
                .into_iter()
                .map(|model| (model.key, model.value))
                .collect();

            let snapshot = GamePlayerPlayerDataSnapshot { data: player_data };

            Ok(GamePlayer::new(
                player,
                Arc::downgrade(&req.state),
                snapshot,
            ))
        })
    }
}

impl FromPacketRequest for SessionAuth {
    type Rejection = BlazeError;

    fn from_packet_request<'a>(
        req: &'a mut PacketRequest,
    ) -> BoxFuture<'a, Result<Self, Self::Rejection>>
    where
        Self: 'a,
    {
        Box::pin(async move {
            let player = req
                .state
                .data
                .get_player()
                .ok_or(GlobalError::AuthenticationRequired)?;

            Ok(SessionAuth(player))
        })
    }
}

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
        req: &'a mut PacketRequest,
    ) -> BoxFuture<'a, Result<Self, Self::Rejection>>
    where
        Self: 'a,
    {
        Box::pin(ready(
            req.packet
                .deserialize::<'a, V>()
                .map_err(|err| {
                    error!("Error while decoding packet: {:?}", err);
                    GlobalError::System.into()
                })
                .map(Blaze),
        ))
    }
}

impl FromPacketRequest for SessionLink {
    type Rejection = Infallible;

    fn from_packet_request<'a>(
        req: &'a mut PacketRequest,
    ) -> BoxFuture<'a, Result<Self, Self::Rejection>>
    where
        Self: 'a,
    {
        let state = req.state.clone();
        Box::pin(ready(Ok(state)))
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

impl<T, E> IntoPacketResponse for Result<T, E>
where
    T: IntoPacketResponse,
    E: IntoPacketResponse,
{
    fn into_response(self, req: &Packet) -> Packet {
        match self {
            Ok(value) => value.into_response(req),
            Err(value) => value.into_response(req),
        }
    }
}

impl<V> IntoPacketResponse for Option<V>
where
    V: IntoPacketResponse,
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
        impl<Fun, Fut, $($ty,)* Res> Handler<($($ty,)*), Res> for Fun
        where
            Fun: Fn($($ty),*) -> Fut + Send + Sync + 'static,
            Fut: Future<Output = Res> + Send,
            $( $ty: FromPacketRequest + Send, )*
            Res: IntoPacketResponse,
        {
            fn handle(&self, req: PacketRequest) -> BoxFuture<'_, Packet>
            {
                Box::pin(async move {
                    let mut req = req;
                    $(

                        let $ty = match $ty::from_packet_request(&mut req).await {
                            Ok(value) => value,
                            Err(rejection) => return rejection.into_response(&req.packet),
                        };
                    )*

                    let res = self($($ty),* ).await;
                    res.into_response(&req.packet)
                })
            }
        }
    };
}

// Implement a handler for every tuple
all_the_tuples!(impl_handler);
