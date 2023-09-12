//! Sessions are client connections to the main server with associated
//! data such as player data for when they become authenticated and
//! networking data.

use self::{
    models::user_sessions::{
        HardwareFlags, LookupResponse, NotifyUserAdded, NotifyUserRemoved, NotifyUserUpdated,
        UserDataFlags, UserIdentification, UserSessionExtendedData, UserSessionExtendedDataUpdate,
    },
    packet::{Packet, PacketDebug},
    router::BlazeRouter,
};
use crate::{
    database::entities::Player,
    middleware::blaze_upgrade::BlazeScheme,
    services::{
        game::{manager::GameManager, GamePlayer},
        sessions::Sessions,
    },
    session::models::{NetworkAddress, Port, QosNetworkData},
    utils::{
        components::{self, user_sessions},
        types::{GameID, PlayerID, SessionID},
    },
};
use interlink::prelude::*;
use log::{debug, log_enabled};
use serde::Serialize;
use std::{fmt::Debug, io, net::Ipv4Addr, sync::Arc};

pub mod models;
pub mod packet;
pub mod router;
pub mod routes;

/// Structure for storing a client session. This includes the
/// network stream for the client along with global state and
/// other session state.
pub struct Session {
    /// Unique identifier for this session.
    id: SessionID,
    /// Connection socket addr
    addr: Ipv4Addr,
    /// Packet writer sink for the session
    writer: SinkLink<Packet>,
    /// The session scheme
    host_target: SessionHostTarget,

    /// Data associated with this session
    data: Option<SessionExtData>,

    router: Arc<BlazeRouter>,

    game_manager: Arc<GameManager>,
    sessions: Arc<Sessions>,
}

pub struct SessionExtData {
    /// The authenticated player
    player: Arc<Player>,
    /// Networking information
    net: Arc<NetData>,
    /// The id of the game if connected to one
    game: Option<GameID>,
    /// Sessions that are subscribed to changes for this session data
    subscribers: Vec<(PlayerID, SessionLink)>,
}

#[derive(Message)]
pub enum SubscriberMessage {
    Sub(PlayerID, SessionLink),
    Remove(PlayerID),
}

impl Handler<SubscriberMessage> for Session {
    type Response = ();

    fn handle(
        &mut self,
        msg: SubscriberMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        let data = match &mut self.data {
            Some(value) => value,
            // TODO: Handle this as an error for unauthenticated
            None => return,
        };

        match msg {
            SubscriberMessage::Sub(player_id, subscriber) => {
                data.add_subscriber(player_id, subscriber)
            }
            SubscriberMessage::Remove(player_id) => data.remove_subscriber(player_id),
        }
    }
}

impl SessionExtData {
    pub fn new(player: Player) -> Self {
        Self {
            player: Arc::new(player),
            net: Default::default(),
            game: Default::default(),
            subscribers: Default::default(),
        }
    }

    fn ext(&self) -> UserSessionExtendedData {
        UserSessionExtendedData {
            net: self.net.clone(),
            game: self.game,
        }
    }

    fn add_subscriber(&mut self, player_id: PlayerID, subscriber: SessionLink) {
        // Create the details packets
        let added_notify = Packet::notify(
            user_sessions::COMPONENT,
            user_sessions::USER_ADDED,
            NotifyUserAdded {
                session_data: self.ext(),
                user: UserIdentification::from_player(&self.player),
            },
        );

        // Create update notifying the user of the subscription
        let update_notify = Packet::notify(
            user_sessions::COMPONENT,
            user_sessions::USER_UPDATED,
            NotifyUserUpdated {
                flags: UserDataFlags::SUBSCRIBED | UserDataFlags::ONLINE,
                player_id: self.player.id,
            },
        );

        self.subscribers.push((player_id, subscriber.clone()));
        subscriber.push(added_notify);
        subscriber.push(update_notify);
    }

    fn remove_subscriber(&mut self, player_id: PlayerID) {
        let index = match self.subscribers.iter().position(|(id, _)| player_id.eq(id)) {
            Some(value) => value,
            None => return,
        };

        let (_, subscriber) = self.subscribers.swap_remove(index);

        // Create the details packets
        let removed_notify = Packet::notify(
            user_sessions::COMPONENT,
            user_sessions::USER_REMOVED,
            NotifyUserRemoved { player_id },
        );

        subscriber.push(removed_notify)
    }

    /// Publishes changes of the session data to all the
    /// subscribed session links
    fn publish_update(&self) {
        let packet = Packet::notify(
            user_sessions::COMPONENT,
            user_sessions::USER_SESSION_EXTENDED_DATA_UPDATE,
            UserSessionExtendedDataUpdate {
                user_id: self.player.id,
                data: self.ext(),
            },
        );

        for (_, subscriber) in &self.subscribers {
            subscriber.push(packet.clone());
        }
    }
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct NetData {
    pub addr: NetworkAddress,
    pub qos: QosNetworkData,
    pub hardware_flags: HardwareFlags,
}

impl NetData {
    // Re-creates the current net data using the provided address and QOS data
    pub fn with_basic(&self, addr: NetworkAddress, qos: QosNetworkData) -> Self {
        Self {
            addr,
            qos,
            hardware_flags: self.hardware_flags,
        }
    }

    /// Re-creates the current net data using the provided hardware flags
    pub fn with_hardware_flags(&self, flags: HardwareFlags) -> Self {
        Self {
            addr: self.addr.clone(),
            qos: self.qos,
            hardware_flags: flags,
        }
    }
}

impl Service for Session {
    fn stopping(&mut self) {
        self.clear_auth();
        debug!("Session stopped (SID: {})", self.id);
    }
}

pub type SessionLink = Link<Session>;

#[derive(Message)]
#[msg(rtype = "Option<Arc<Player>>")]
pub struct GetPlayerMessage;

impl Handler<GetPlayerMessage> for Session {
    type Response = Mr<GetPlayerMessage>;

    fn handle(
        &mut self,
        _msg: GetPlayerMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        Mr(self.data.as_ref().map(|data| data.player.clone()))
    }
}

#[derive(Message)]
#[msg(rtype = "SessionHostTarget")]
pub struct GetHostTarget;

impl Handler<GetHostTarget> for Session {
    type Response = Mr<GetHostTarget>;

    fn handle(&mut self, _msg: GetHostTarget, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        Mr(self.host_target.clone())
    }
}

#[derive(Clone)]
pub struct SessionHostTarget {
    pub scheme: BlazeScheme,
    pub host: Box<str>,
    pub port: Port,
    pub local_http: bool,
}

#[derive(Message)]
#[msg(rtype = "Option<GamePlayer>")]
pub struct GetGamePlayerMessage;

impl Handler<GetGamePlayerMessage> for Session {
    type Response = Mr<GetGamePlayerMessage>;
    fn handle(
        &mut self,
        _msg: GetGamePlayerMessage,
        ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        Mr(self
            .data
            .as_ref()
            .map(|data| GamePlayer::new(data.player.clone(), data.net.clone(), ctx.link())))
    }
}

#[derive(Message)]
pub struct SetPlayerMessage(pub Option<Player>);

impl Handler<SetPlayerMessage> for Session {
    type Response = ();
    fn handle(&mut self, msg: SetPlayerMessage, ctx: &mut ServiceContext<Self>) -> Self::Response {
        debug_assert!(
            self.data.is_none(),
            "Attempted to set player on session that already has a player"
        );

        // Clear the current authentication
        // TODO: Handle already authenticated as error and close session
        // rather than re-resuming it
        self.clear_auth();

        // If we are setting a new player
        if let Some(player) = msg.0 {
            let sessions = self.sessions.clone();
            let player_id = player.id;
            let link = ctx.link();
            // Add the session to authenticated sessions
            tokio::spawn(async move {
                sessions.add_session(player_id, link).await;
            });

            let data = SessionExtData::new(player);
            self.data = Some(data);
        }
    }
}

/// Extension for links to push packets for session links
pub trait PushExt {
    fn push(&self, packet: Packet);
}

impl PushExt for Link<Session> {
    #[inline]
    fn push(&self, packet: Packet) {
        let _ = self.do_send(WriteMessage(packet));
    }
}

#[derive(Message)]
#[msg(rtype = "Option<GameID>")]
pub struct GetPlayerGameMessage;

impl Handler<GetPlayerGameMessage> for Session {
    type Response = Mr<GetPlayerGameMessage>;

    fn handle(
        &mut self,
        _msg: GetPlayerGameMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        Mr(self.data.as_ref().and_then(|data| data.game))
    }
}

#[derive(Message)]
#[msg(rtype = "Option<LookupResponse>")]
pub struct GetLookupMessage;

impl Handler<GetLookupMessage> for Session {
    type Response = Mr<GetLookupMessage>;

    fn handle(
        &mut self,
        _msg: GetLookupMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        Mr(self.data.as_ref().map(|data| LookupResponse {
            player: data.player.clone(),
            extended_data: UserSessionExtendedData {
                net: data.net.clone(),
                game: data.game,
            },
        }))
    }
}

#[derive(Message)]
pub struct WriteMessage(pub Packet);

impl Handler<WriteMessage> for Session {
    type Response = ();

    fn handle(&mut self, msg: WriteMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        self.push(msg.0);
    }
}

impl StreamHandler<io::Result<Packet>> for Session {
    fn handle(&mut self, msg: io::Result<Packet>, ctx: &mut ServiceContext<Self>) {
        if let Ok(packet) = msg {
            self.debug_log_packet("Read", &packet);
            let addr = ctx.link();
            let router = self.router.clone();
            tokio::spawn(async move {
                let response = match router.handle(addr.clone(), packet) {
                    // Await the handler response future
                    Ok(fut) => fut.await,

                    // Handle no handler for packet
                    Err(packet) => {
                        debug!("Missing packet handler");
                        Packet::response_empty(&packet)
                    }
                };
                // Push the response to the client
                addr.push(response);
            });
        } else {
            ctx.stop();
        }
    }
}

impl ErrorHandler<io::Error> for Session {
    fn handle(&mut self, _err: io::Error, _ctx: &mut ServiceContext<Self>) -> ErrorAction {
        ErrorAction::Continue
    }
}

#[derive(Message)]
#[msg(rtype = "Ipv4Addr")]
pub struct GetSocketAddrMessage;

impl Handler<GetSocketAddrMessage> for Session {
    type Response = Mr<GetSocketAddrMessage>;

    fn handle(
        &mut self,
        _msg: GetSocketAddrMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        Mr(self.addr)
    }
}

/// Message to update the hardware flag of a session
#[derive(Message)]
pub struct HardwareFlagMessage {
    /// The new value for the hardware flag
    pub value: HardwareFlags,
}

impl Handler<HardwareFlagMessage> for Session {
    type Response = ();

    fn handle(&mut self, msg: HardwareFlagMessage, _ctx: &mut ServiceContext<Self>) {
        let data = match &mut self.data {
            Some(value) => value,
            // TODO: Handle this as an error for unauthenticated
            None => return,
        };

        data.net = Arc::new(data.net.with_hardware_flags(msg.value));
        data.publish_update();
    }
}

#[derive(Message)]
pub struct NetworkInfoMessage {
    pub address: NetworkAddress,
    pub qos: QosNetworkData,
}

impl Handler<NetworkInfoMessage> for Session {
    type Response = ();

    fn handle(&mut self, msg: NetworkInfoMessage, _ctx: &mut ServiceContext<Self>) {
        let data = match &mut self.data {
            Some(value) => value,
            // TODO: Handle this as an error for unauthenticated
            None => return,
        };

        data.net = Arc::new(data.net.with_basic(msg.address, msg.qos));
        data.publish_update();
    }
}

#[derive(Message)]
pub struct SetGameMessage {
    pub game: Option<GameID>,
}

impl Handler<SetGameMessage> for Session {
    type Response = ();

    fn handle(&mut self, msg: SetGameMessage, _ctx: &mut ServiceContext<Self>) {
        let data = match &mut self.data {
            Some(value) => value,
            // TODO: Handle this as an error for unauthenticated
            None => return,
        };

        data.game = msg.game;
        data.publish_update();
    }
}

impl Session {
    pub fn new(
        id: SessionID,
        host_target: SessionHostTarget,
        writer: SinkLink<Packet>,
        addr: Ipv4Addr,
        router: Arc<BlazeRouter>,
        game_manager: Arc<GameManager>,
        sessions: Arc<Sessions>,
    ) -> Self {
        Self {
            id,
            writer,
            data: None,
            host_target,
            addr,
            router,
            game_manager,
            sessions,
        }
    }

    /// Pushes a new packet to the back of the packet buffer
    /// and sends a flush notification
    ///
    /// `packet` The packet to push to the buffer
    pub fn push(&self, packet: Packet) {
        self.debug_log_packet("Queued Write", &packet);
        if self.writer.sink(packet).is_err() {
            // TODO: Handle failing to send contents to writer
        }
    }

    /// Logs the contents of the provided packet to the debug output along with
    /// the header information and basic session information.
    ///
    /// `action` The name of the action this packet is undergoing.
    ///          (e.g. Writing or Reading)
    /// `packet` The packet that is being logged
    fn debug_log_packet(&self, action: &'static str, packet: &Packet) {
        // Skip if debug logging is disabled
        if !log_enabled!(log::Level::Debug) {
            return;
        }

        // Ping messages are ignored from debug logging as they are very frequent
        let ignored = packet.header.component == components::util::COMPONENT
            && (packet.header.command == components::util::PING
                || packet.header.command == components::util::SUSPEND_USER_PING);

        if ignored {
            return;
        }

        let debug = SessionPacketDebug {
            action,
            packet,
            session: self,
        };

        debug!("\n{:?}", debug);
    }

    /// Removes the session from any connected games and the
    /// matchmaking queue
    pub fn remove_games(&mut self) {
        // Don't attempt to remove if theres no active player
        let data = match &mut self.data {
            Some(value) => value,
            None => return,
        };

        let game_manager = self.game_manager.clone();
        let game = data.game.take();
        let player_id = data.player.id;
        tokio::spawn(async move {
            game_manager.remove_session(game, player_id).await;
        });
    }

    /// Removes the player from the authenticated sessions list
    /// if the player is authenticated
    pub fn clear_auth(&mut self) {
        self.remove_games();

        // Check that theres authentication
        let data = match &self.data {
            Some(value) => value,
            None => return,
        };
        let player_id = data.player.id;

        // Remove the session from the sessions service
        let sessions = self.sessions.clone();
        tokio::spawn(async move {
            sessions.remove_session(player_id).await;
        });
    }
}

/// Structure for wrapping session details around a debug
/// packet message for logging
struct SessionPacketDebug<'a> {
    action: &'static str,
    packet: &'a Packet,
    session: &'a Session,
}

impl Debug for SessionPacketDebug<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Session {} Packet", self.action)?;

        if let Some(data) = &self.session.data {
            writeln!(
                f,
                "Info: (Name: {}, ID: {}, SID: {})",
                &data.player.display_name, data.player.id, &self.session.id
            )?;
        } else {
            writeln!(f, "Info: (SID: {})", &self.session.id)?;
        }

        let header = &self.packet.header;

        let minified = (header.component == components::authentication::COMPONENT
            && header.command == components::authentication::LIST_USER_ENTITLEMENTS_2)
            || (header.component == components::util::COMPONENT
                && (header.command == components::util::FETCH_CLIENT_CONFIG
                    || header.command == components::util::USER_SETTINGS_LOAD_ALL));

        PacketDebug {
            packet: self.packet,
            minified,
        }
        .fmt(f)
    }
}
