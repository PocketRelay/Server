//! Sessions are client connections to the main server with associated
//! data such as player data for when they become authenticated and
//! networking data.

use crate::{
    database::entities::Player,
    middleware::blaze_upgrade::BlazeScheme,
    services::{
        game::{
            manager::RemovePlayerMessage, models::RemoveReason, player::GamePlayer,
            RemovePlayerType,
        },
        matchmaking::RemoveQueueMessage,
        sessions::{AddMessage, RemoveMessage},
        Services,
    },
    state::GlobalState,
    utils::{
        components::{self, Components, UserSessions},
        models::{NetData, NetGroups, Port, QosNetworkData, UpdateExtDataAttr},
        types::{GameID, PlayerID, SessionID},
    },
};
use blaze_pk::{
    codec::Encodable,
    packet::{Packet, PacketComponents, PacketDebug},
    router::HandleError,
    tag::TdfType,
    value_type,
    writer::TdfWriter,
};
use interlink::prelude::*;
use log::{debug, error, log_enabled};
use std::{fmt::Debug, io};

pub mod models;
pub mod routes;

/// Structure for storing a client session. This includes the
/// network stream for the client along with global state and
/// other session state.
pub struct Session {
    /// Unique identifier for this session.
    id: SessionID,

    /// Packet writer sink for the session
    writer: SinkLink<Packet>,

    /// The session scheme
    host_target: SessionHostTarget,

    /// Data associated with this session
    data: SessionData,
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

impl Service for Session {
    fn stopping(&mut self) {
        let services = GlobalState::services();
        self.remove_games(services);
        self.clear_auth(services);
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
    pub host: String,
    pub port: Port,
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
            self.id,
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
        let services = GlobalState::services();

        // Take the old player and remove it if possible
        if let Some(old_player) = self.data.player.take() {
            let _ = services.sessions.do_send(RemoveMessage {
                player_id: old_player.id,
            });
        }

        // If we are setting a new player
        if let Some(player) = msg.0 {
            // Add the session to authenticated sessions
            let _ = services.sessions.do_send(AddMessage {
                player_id: player.id,
                link: ctx.link(),
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
#[msg(rtype = "SessionID")]
pub struct GetIdMessage;

impl Handler<GetIdMessage> for Session {
    type Response = Mr<GetIdMessage>;

    fn handle(&mut self, _msg: GetIdMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        Mr(self.id)
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
            let mut addr = ctx.link();
            tokio::spawn(async move {
                let router = GlobalState::router();
                let response = match router.handle(&mut addr, packet) {
                    // Await the handler response future
                    Ok(fut) => fut.await,

                    // Handle any errors that occur
                    Err(err) => {
                        match err {
                            // No handler set-up just respond with a default empty response
                            HandleError::MissingHandler(packet) => packet.respond_empty(),
                            HandleError::Decoding(err) => {
                                error!("Error while decoding packet: {:?}", err);
                                return;
                            }
                        }
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
                Components::UserSessions(UserSessions::SetSession),
                SetSession {
                    player_id: player.id,
                    session: self,
                },
            );
            self.push(packet);
        }
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
                Components::UserSessions(UserSessions::SetSession),
                SetSession {
                    player_id: player.id,
                    session: self,
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
    pub groups: NetGroups,
    pub qos: QosNetworkData,
}

impl Handler<NetworkInfoMessage> for Session {
    type Response = ();

    fn handle(&mut self, msg: NetworkInfoMessage, ctx: &mut ServiceContext<Self>) {
        let net = &mut &mut self.data.net;
        net.qos = msg.qos;
        net.groups = Some(msg.groups);

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
            Components::UserSessions(UserSessions::SessionDetails),
            SessionUpdate {
                session: self,
                player_id: player.id,
                display_name: &player.display_name,
            },
        );

        let b = Packet::notify(
            Components::UserSessions(UserSessions::UpdateExtendedDataAttribute),
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
    /// Creates a new session with the provided values.
    ///
    /// `id`             The unique session ID
    /// `values`         The networking TcpStream and address
    /// `message_sender` The message sender for session messages
    pub fn new(id: SessionID, host_target: SessionHostTarget, writer: SinkLink<Packet>) -> Self {
        Self {
            id,
            writer,
            data: SessionData::default(),
            host_target,
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

        let component = Components::from_header(&packet.header);

        // Ping messages are ignored from debug logging as they are very frequent
        let ignored = if let Some(component) = &component {
            matches!(
                component,
                Components::Util(components::Util::Ping)
                    | Components::Util(components::Util::SuspendUserPing)
            )
        } else {
            false
        };

        if ignored {
            return;
        }

        let debug = SessionPacketDebug {
            action,
            packet,
            component,
            session: self,
        };

        debug!("\n{:?}", debug);
    }

    /// Removes the session from any connected games and the
    /// matchmaking queue
    pub fn remove_games(&mut self, services: &Services) {
        let game = self.data.game.take();
        let _ = if let Some(game_id) = game {
            services.game_manager.do_send(RemovePlayerMessage {
                game_id,
                id: self.id,
                reason: RemoveReason::Generic,
                ty: RemovePlayerType::Session,
            })
        } else {
            services.matchmaking.do_send(RemoveQueueMessage {
                session_id: self.id,
            })
        };
    }

    /// Removes the player from the authenticated sessions list
    /// if the player is authenticated
    pub fn clear_auth(&mut self, services: &Services) {
        // Check that theres authentication
        let player = match &self.data.player {
            Some(value) => value,
            None => return,
        };

        // Send the remove session message
        let _ = services.sessions.do_send(RemoveMessage {
            player_id: player.id,
        });
    }
}

/// Structure for wrapping session details around a debug
/// packet message for logging
struct SessionPacketDebug<'a> {
    action: &'static str,
    packet: &'a Packet,
    component: Option<Components>,
    session: &'a Session,
}

impl Debug for SessionPacketDebug<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Session {} Packet", self.action)?;

        let component = &self.component;

        if let Some(player) = &self.session.data.player {
            writeln!(
                f,
                "Info: (Name: {}, ID: {}, SID: {})",
                &player.display_name, &player.id, &self.session.id
            )?;
        } else {
            writeln!(f, "Info: ( SID: {})", &self.session.id)?;
        }

        let minified = if let Some(component) = &self.component {
            matches!(
                component,
                Components::Authentication(components::Authentication::ListUserEntitlements2)
                    | Components::Util(components::Util::FetchClientConfig)
                    | Components::Util(components::Util::UserSettingsLoadAll)
            )
        } else {
            false
        };

        PacketDebug {
            packet: self.packet,
            component: component.as_ref(),
            minified,
        }
        .fmt(f)
    }
}

impl Encodable for SessionData {
    fn encode(&self, writer: &mut TdfWriter) {
        self.net.tag_groups(b"ADDR", writer);
        writer.tag_str(b"BPS", "ea-sjc");
        writer.tag_str_empty(b"CTY");
        writer.tag_var_int_list_empty(b"CVAR");
        {
            writer.tag_map_start(b"DMAP", TdfType::VarInt, TdfType::VarInt, 1);
            writer.write_u32(0x70001);
            writer.write_u16(0x409a);
        }
        writer.tag_u16(b"HWFG", self.net.hardware_flags);
        {
            // Ping latency to the Quality of service servers
            writer.tag_list_start(b"PSLM", TdfType::VarInt, 1);
            0xfff0fff.encode(writer);
        }
        writer.tag_value(b"QDAT", &self.net.qos);
        writer.tag_u8(b"UATT", 0);
        if let Some(game_id) = &self.game {
            writer.tag_list_start(b"ULST", TdfType::Triple, 1);
            (4, 1, *game_id).encode(writer);
        }
        writer.tag_group_end();
    }
}

value_type!(SessionData, TdfType::Group);

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

impl Encodable for SessionUpdate<'_> {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_value(b"DATA", &self.session.data);

        writer.tag_group(b"USER");
        writer.tag_u32(b"AID", self.player_id);
        writer.tag_u32(b"ALOC", 0x64654445);
        writer.tag_empty_blob(b"EXBB");
        writer.tag_u8(b"EXID", 0);
        writer.tag_u32(b"ID", self.player_id);
        writer.tag_str(b"NAME", self.display_name);
        writer.tag_group_end();
    }
}

pub struct LookupResponse {
    session_data: SessionData,
    player_id: PlayerID,
    display_name: String,
}

impl Encodable for LookupResponse {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_value(b"EDAT", &self.session_data);

        writer.tag_u8(b"FLGS", 2);

        writer.tag_group(b"USER");
        writer.tag_u32(b"AID", self.player_id);
        writer.tag_u32(b"ALOC", 0x64654445);
        writer.tag_empty_blob(b"EXBB");
        writer.tag_u8(b"EXID", 0);
        writer.tag_u32(b"ID", self.player_id);
        writer.tag_str(b"NAME", &self.display_name);
        writer.tag_group_end();
    }
}

/// Session update for ourselves
struct SetSession<'a> {
    /// The player ID the update is for
    player_id: PlayerID,
    /// The session this update is for
    session: &'a Session,
}

impl Encodable for SetSession<'_> {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_value(b"DATA", &self.session.data);
        writer.tag_u32(b"USID", self.player_id);
    }
}
