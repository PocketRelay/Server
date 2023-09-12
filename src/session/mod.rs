//! Sessions are client connections to the main server with associated
//! data such as player data for when they become authenticated and
//! networking data.

use self::{
    packet::{Packet, PacketDebug},
    router::BlazeRouter,
};
use crate::{
    database::entities::Player,
    middleware::blaze_upgrade::BlazeScheme,
    services::{
        game::{manager::GameManager, models::RemoveReason, GamePlayer, RemovePlayerMessage},
        sessions::Sessions,
    },
    utils::{
        components::{self, game_manager::GAME_TYPE, user_sessions},
        models::{NetworkAddress, Port, QosNetworkData, UpdateExtDataAttr},
        types::{GameID, PlayerID, SessionID},
    },
};
use interlink::prelude::*;
use log::{debug, log_enabled};
use serde::Serialize;
use std::{fmt::Debug, io, net::Ipv4Addr, sync::Arc};
use tdf::{ObjectId, TdfSerialize, TdfType, TdfTyped};

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
    data: SessionData,

    router: Arc<BlazeRouter>,

    game_manager: Arc<GameManager>,
    sessions: Arc<Sessions>,
}

#[derive(Default, Clone)]
pub struct SessionData {
    /// If the session is authenticated it will have a linked
    /// player model from the database
    player: Option<Player>,
    /// Networking information
    net: NetData,
    /// The id of the game if connected to one
    game: Option<GameID>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct NetData {
    pub addr: NetworkAddress,
    pub qos: QosNetworkData,
    pub hardware_flags: u16,
}

impl Service for Session {
    fn stopping(&mut self) {
        self.clear_auth();
        debug!("Session stopped (SID: {})", self.id);
    }
}

pub type SessionLink = Link<Session>;

#[derive(Message)]
#[msg(rtype = "Option<Player>")]
pub struct GetPlayerMessage;

impl Handler<GetPlayerMessage> for Session {
    type Response = Mr<GetPlayerMessage>;

    fn handle(
        &mut self,
        _msg: GetPlayerMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        Mr(self.data.player.clone())
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
#[msg(rtype = "Option<u32>")]
pub struct GetPlayerIdMessage;

impl Handler<GetPlayerIdMessage> for Session {
    type Response = Mr<GetPlayerIdMessage>;

    fn handle(
        &mut self,
        _msg: GetPlayerIdMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        Mr(self.data.player.as_ref().map(|value| value.id))
    }
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
        let player = match self.data.player.clone() {
            Some(value) => value,
            None => return Mr(None),
        };
        Mr(Some(GamePlayer::new(
            player,
            self.data.net.clone(),
            ctx.link(),
        )))
    }
}

#[derive(Message)]
pub struct SetPlayerMessage(pub Option<Player>);

impl Handler<SetPlayerMessage> for Session {
    type Response = ();
    fn handle(&mut self, msg: SetPlayerMessage, ctx: &mut ServiceContext<Self>) -> Self::Response {
        // Clear the current authentication
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

            self.data.player = Some(player);
        }
    }
}

/// Extension for links to push packets for session links
pub trait PushExt {
    fn push(&self, packet: Packet);
}

impl PushExt for Link<Session> {
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
        Mr(self.data.game)
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
        let data = &self.data;
        let player = match &data.player {
            Some(value) => value,
            None => return Mr(None),
        };

        let response = LookupResponse {
            session_data: data.clone(),
            player_id: player.id,
            display_name: player.display_name.clone(),
        };

        Mr(Some(response))
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

/// Message telling the session to inform the clients of
/// a change in session data
#[derive(Message)]
pub struct UpdateClientMessage;

impl Handler<UpdateClientMessage> for Session {
    type Response = ();

    fn handle(&mut self, _msg: UpdateClientMessage, _ctx: &mut ServiceContext<Self>) {
        if let Some(player) = &self.data.player {
            let packet = Packet::notify(
                user_sessions::COMPONENT,
                user_sessions::SET_SESSION,
                SetSession {
                    player_id: player.id,
                    session: &self.data,
                },
            );
            self.push(packet);
        }
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

/// Creates a set session packet and sends it to all the
/// provided session links
#[derive(Message)]
pub struct InformSessions {
    /// The link to send the set session to
    pub links: Vec<Link<Session>>,
}

impl Handler<InformSessions> for Session {
    type Response = ();

    fn handle(&mut self, msg: InformSessions, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        if let Some(player) = &self.data.player {
            let packet = Packet::notify(
                user_sessions::COMPONENT,
                user_sessions::SET_SESSION,
                SetSession {
                    player_id: player.id,
                    session: &self.data,
                },
            );

            for link in msg.links {
                link.push(packet.clone());
            }
        }
    }
}

/// Message to update the hardware flag of a session
#[derive(Message)]
pub struct HardwareFlagMessage {
    /// The new value for the hardware flag
    pub value: u16,
}

impl Handler<HardwareFlagMessage> for Session {
    type Response = ();

    fn handle(&mut self, msg: HardwareFlagMessage, ctx: &mut ServiceContext<Self>) {
        self.data.net.hardware_flags = msg.value;

        // Notify the client of the change via a message rather than
        // directly so its sent after the response
        let _ = ctx.shared_link().do_send(UpdateClientMessage);
    }
}

#[derive(Message)]
pub struct NetworkInfoMessage {
    pub address: NetworkAddress,
    pub qos: QosNetworkData,
}

impl Handler<NetworkInfoMessage> for Session {
    type Response = ();

    fn handle(&mut self, msg: NetworkInfoMessage, ctx: &mut ServiceContext<Self>) {
        let net = &mut &mut self.data.net;
        net.qos = msg.qos;
        net.addr = msg.address;

        // Notify the client of the change via a message rather than
        // directly so its sent after the response
        let _ = ctx.shared_link().do_send(UpdateClientMessage);
    }
}

#[derive(Message)]
pub struct SetGameMessage {
    pub game: Option<GameID>,
}

impl Handler<SetGameMessage> for Session {
    type Response = ();

    fn handle(&mut self, msg: SetGameMessage, ctx: &mut ServiceContext<Self>) {
        self.data.game = msg.game;

        // Notify the client of the change via a message rather than
        // directly so its sent after the response
        let _ = ctx.shared_link().do_send(UpdateClientMessage);
    }
}

/// Message to send the details of this session to
/// the provided session link
#[derive(Message)]
pub struct DetailsMessage {
    pub link: Link<Session>,
}

impl Handler<DetailsMessage> for Session {
    type Response = ();

    fn handle(&mut self, msg: DetailsMessage, _ctx: &mut ServiceContext<Self>) {
        let player = match self.data.player.as_ref() {
            Some(value) => value,
            None => return,
        };

        // Create the details packets
        let a = Packet::notify(
            user_sessions::COMPONENT,
            user_sessions::SESSION_DETAILS,
            SessionUpdate {
                session: self,
                player_id: player.id,
                display_name: &player.display_name,
            },
        );

        let b = Packet::notify(
            user_sessions::COMPONENT,
            user_sessions::UPDATE_EXTENDED_DATA_ATTRIBUTE,
            UpdateExtDataAttr {
                flags: 0x3,
                player_id: player.id,
            },
        );

        // Push the message to the session link
        msg.link.push(a);
        msg.link.push(b);
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
            data: SessionData::default(),
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
    pub fn push(&mut self, packet: Packet) {
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
        let player_id = match &self.data.player {
            Some(value) => value.id,
            None => return,
        };

        let game_manager = self.game_manager.clone();

        if let Some(game_id) = self.data.game.take() {
            // Remove the player from the game
            tokio::spawn(async move {
                // Obtain the current game
                let game = match game_manager.get_game(game_id).await {
                    Some(value) => value,
                    None => return,
                };

                // Send the remove message
                let _ = game
                    .send(RemovePlayerMessage {
                        id: player_id,
                        reason: RemoveReason::PlayerLeft,
                    })
                    .await;
            });
        } else {
            // Remove the player from matchmaking if present
            tokio::spawn(async move {
                game_manager.remove_queue(player_id).await;
            });
        }
    }

    /// Removes the player from the authenticated sessions list
    /// if the player is authenticated
    pub fn clear_auth(&mut self) {
        self.remove_games();

        // Check that theres authentication
        let player = match self.data.player.take() {
            Some(value) => value,
            None => return,
        };

        // Remove the session from the sessions service
        let sessions = self.sessions.clone();
        tokio::spawn(async move {
            sessions.remove_session(player.id).await;
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

        if let Some(player) = &self.session.data.player {
            writeln!(
                f,
                "Info: (Name: {}, ID: {}, SID: {})",
                &player.display_name, &player.id, &self.session.id
            )?;
        } else {
            writeln!(f, "Info: ( SID: {})", &self.session.id)?;
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

impl TdfSerialize for SessionData {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.group_body(|w| {
            w.tag_ref(b"ADDR", &self.net.addr);
            w.tag_str(b"BPS", "ea-sjc");
            w.tag_str_empty(b"CTY");
            w.tag_var_int_list_empty(b"CVAR");

            w.tag_map_tuples(b"DMAP", &[(0x70001, 0x409a)]);

            w.tag_u16(b"HWFG", self.net.hardware_flags);

            // Ping latency to the Quality of service servers
            w.tag_list_slice(b"PSLM", &[0xfff0fff]);

            w.tag_ref(b"QDAT", &self.net.qos);
            w.tag_u8(b"UATT", 0);
            if let Some(game_id) = &self.game {
                w.tag_list_slice(b"ULST", &[ObjectId::new(GAME_TYPE, *game_id as u64)]);
            }
        });
    }
}

impl TdfTyped for SessionData {
    const TYPE: TdfType = TdfType::Group;
}

/// Session update for a session other than ourselves
/// which contains the details for that session
struct SessionUpdate<'a> {
    /// The session this update is for
    session: &'a Session,
    /// The player ID the update is for
    player_id: PlayerID,
    /// The display name of the player the update is
    display_name: &'a str,
}

impl TdfSerialize for SessionUpdate<'_> {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.tag_ref(b"DATA", &self.session.data);

        w.group(b"USER", |writer| {
            writer.tag_owned(b"AID", self.player_id);
            writer.tag_u32(b"ALOC", 0x64654445);
            writer.tag_blob_empty(b"EXBB");
            writer.tag_u8(b"EXID", 0);
            writer.tag_owned(b"ID", self.player_id);
            writer.tag_str(b"NAME", self.display_name);
        });
    }
}

pub struct LookupResponse {
    session_data: SessionData,
    player_id: PlayerID,
    display_name: String,
}

impl TdfSerialize for LookupResponse {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.tag_ref(b"EDAT", &self.session_data);

        w.tag_u8(b"FLGS", 2);

        w.group(b"USER", |w| {
            w.tag_owned(b"AID", self.player_id);
            w.tag_u32(b"ALOC", 0x64654445);
            w.tag_blob_empty(b"EXBB");
            w.tag_u8(b"EXID", 0);
            w.tag_owned(b"ID", self.player_id);
            w.tag_str(b"NAME", &self.display_name);
        });
    }
}

/// Session update for ourselves
struct SetSession<'a> {
    /// The session this update is for
    session: &'a SessionData,
    /// The player ID the update is for
    player_id: PlayerID,
}

impl TdfSerialize for SetSession<'_> {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.tag_ref(b"DATA", self.session);
        w.tag_owned(b"USID", self.player_id)
    }
}
