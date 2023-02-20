//! Sessions are client connections to the main server with associated
//! data such as player data for when they become authenticated and
//! networking data.
use super::models::errors::{ServerError, ServerResult};
use super::router;
use crate::services::game::manager::RemovePlayerMessage;
use crate::services::game::matchmaking::RemoveQueueMessage;
use crate::utils::components;
use crate::utils::types::PlayerID;
use crate::{
    services::game::{player::GamePlayer, RemovePlayerType},
    state::GlobalState,
    utils::{
        components::{Components, UserSessions},
        models::{NetData, NetGroups, QosNetworkData, UpdateExtDataAttr},
        types::{GameID, SessionID},
    },
};
use blaze_pk::packet::PacketDebug;
use blaze_pk::{codec::Encodable, tag::TdfType, writer::TdfWriter};
use blaze_pk::{
    packet::{Packet, PacketComponents},
    router::State,
};
use database::Player;
use interlink::prelude::*;
use log::{debug, error, log_enabled};
use std::fmt::Debug;
use std::io;
use std::net::SocketAddr;

/// Structure for storing a client session. This includes the
/// network stream for the client along with global state and
/// other session state.
pub struct Session {
    /// Unique identifier for this session.
    id: SessionID,

    writer: SinkLink<Packet>,

    /// The socket connection address of the client
    socket_addr: SocketAddr,

    /// If the session is authenticated it will have a linked
    /// player model from the database
    pub player: Option<Player>,

    /// Networking information
    net: NetData,

    /// The id of the game if connected to one
    game: Option<GameID>,
}

impl Service for Session {
    fn stopping(&mut self) {
        self.remove_games();
        debug!("Session stopped (SID: {})", self.id);
    }
}

/// Wrapper over the standard interlink link type
/// to provide extra session functionality and to
/// allow it to be used as State
#[derive(Clone)]
pub struct SessionLink {
    pub link: Link<Session>,
}

impl State for SessionLink {}

impl SessionLink {
    pub fn push(&self, packet: Packet) {
        self.link.do_send(PacketMessage::Write(packet)).ok();
    }

    pub async fn try_into_player(&self) -> Option<GamePlayer> {
        self.link
            .exec(|service, ctx| {
                let player = service.player.clone()?;
                Some(GamePlayer::new(
                    service.id,
                    player,
                    service.net.clone(),
                    SessionLink { link: ctx.link() },
                ))
            })
            .await
            .ok()
            .flatten()
    }

    /// Attempts to set the current player will return true if successful
    /// or false if the sesson is terminated or another error occurs
    ///
    /// `player` The player to set for this session
    pub async fn set_player(&self, player: Player) -> bool {
        self.link
            .exec(|session, _| {
                session.player = Some(player);
            })
            .await
            .is_ok()
    }

    pub async fn get_player(&self) -> ServerResult<Option<Player>> {
        self.link
            .exec(|session, _| session.player.clone())
            .await
            .map_err(|_| ServerError::ServerUnavailable)
    }

    pub async fn get_player_id(&self) -> Option<u32> {
        self.link
            .exec(|session, _| session.player.as_ref().map(|value| value.id))
            .await
            .ok()
            .flatten()
    }
    pub async fn id(&self) -> Option<u32> {
        self.link.exec(|session, _| session.id).await.ok()
    }

    pub async fn socket_addr(&self) -> Option<SocketAddr> {
        self.link.exec(|session, _| session.socket_addr).await.ok()
    }
}

impl Handler<PacketMessage> for Session {
    type Response = ();

    fn handle(
        &mut self,
        msg: PacketMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> <PacketMessage as Message>::Response {
        match msg {
            PacketMessage::Write(packet) => self.push(packet),
        }
    }
}

enum PacketMessage {
    /// Queues a packet to be written to the outbound queue
    Write(Packet),
}

impl Message for PacketMessage {
    type Response = ();
}

impl StreamHandler<io::Result<Packet>> for Session {
    fn handle(&mut self, msg: io::Result<Packet>, ctx: &mut ServiceContext<Self>) {
        if let Ok(packet) = msg {
            self.debug_log_packet("Read", &packet);
            let mut addr = SessionLink { link: ctx.link() };
            tokio::spawn(async move {
                let router = router();
                match router.handle(&mut addr, packet).await {
                    Ok(packet) => {
                        addr.push(packet);
                    }
                    Err(err) => {
                        error!("Error occurred while decoding packet: {:?}", err);
                    }
                }
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
pub struct UpdateClientMessage;

impl Message for UpdateClientMessage {
    type Response = ();
}

impl Handler<UpdateClientMessage> for Session {
    type Response = MessageResponse<UpdateClientMessage>;

    fn handle(
        &mut self,
        _msg: UpdateClientMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        if let Some(player) = &self.player {
            let packet = Packet::notify(
                Components::UserSessions(UserSessions::SetSession),
                SetSession {
                    player_id: player.id,
                    session: self,
                },
            );
            self.push(packet);
        }
        MessageResponse(())
    }
}

/// Message to update the hardware flag of a session
pub struct HardwareFlagMessage {
    /// The new value for the hardware flag
    pub value: u16,
}

impl Message for HardwareFlagMessage {
    type Response = ();
}

impl Handler<HardwareFlagMessage> for Session {
    type Response = MessageResponse<HardwareFlagMessage>;

    fn handle(
        &mut self,
        msg: HardwareFlagMessage,
        ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        self.net.hardware_flags = msg.value;

        // Notify the client of the change via a message rather than
        // directly so its sent after the response
        let _ = ctx.shared_link().do_send(UpdateClientMessage);

        MessageResponse(())
    }
}

pub struct NetworkInfoMessage {
    pub groups: NetGroups,
    pub qos: QosNetworkData,
}

impl Message for NetworkInfoMessage {
    type Response = ();
}

impl Handler<NetworkInfoMessage> for Session {
    type Response = MessageResponse<NetworkInfoMessage>;

    fn handle(
        &mut self,
        msg: NetworkInfoMessage,
        ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        let net = &mut &mut self.net;
        net.qos = msg.qos;
        net.groups = Some(msg.groups);

        // Notify the client of the change via a message rather than
        // directly so its sent after the response
        let _ = ctx.shared_link().do_send(UpdateClientMessage);

        MessageResponse(())
    }
}

pub struct SetGameMessage {
    pub game: Option<GameID>,
}

impl Message for SetGameMessage {
    type Response = ();
}

impl Handler<SetGameMessage> for Session {
    type Response = MessageResponse<SetGameMessage>;

    fn handle(&mut self, msg: SetGameMessage, ctx: &mut ServiceContext<Self>) -> Self::Response {
        self.game = msg.game;

        // Notify the client of the change via a message rather than
        // directly so its sent after the response
        let _ = ctx.shared_link().do_send(UpdateClientMessage);

        MessageResponse(())
    }
}

/// Message to send the details of this session to
/// the provided session link
pub struct DetailsMessage {
    pub link: SessionLink,
}

impl Message for DetailsMessage {
    type Response = ();
}

impl Handler<DetailsMessage> for Session {
    type Response = MessageResponse<DetailsMessage>;

    fn handle(&mut self, msg: DetailsMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        let player = match self.player.as_ref() {
            Some(value) => value,
            None => return MessageResponse(()),
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

        MessageResponse(())
    }
}

impl Session {
    /// Creates a new session with the provided values.
    ///
    /// `id`             The unique session ID
    /// `values`         The networking TcpStream and address
    /// `message_sender` The message sender for session messages
    pub fn new(id: SessionID, socket_addr: SocketAddr, writer: SinkLink<Packet>) -> Self {
        Self {
            id,
            socket_addr,
            writer,
            player: None,
            net: NetData::default(),
            game: None,
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
        let ignored = matches!(
            component,
            Components::Util(components::Util::Ping)
                | Components::Util(components::Util::SuspendUserPing)
        );

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
    pub fn remove_games(&mut self) {
        let game = self.game.take();
        let services = GlobalState::services();
        let _ = if let Some(game_id) = game {
            services.game_manager.do_send(RemovePlayerMessage {
                game_id,
                ty: RemovePlayerType::Session(self.id),
            })
        } else {
            services.matchmaking.do_send(RemoveQueueMessage {
                session_id: self.id,
            })
        };
    }
}

/// Structure for wrapping session details around a debug
/// packet message for logging
struct SessionPacketDebug<'a> {
    action: &'static str,
    packet: &'a Packet,
    component: Components,
    session: &'a Session,
}

impl Debug for SessionPacketDebug<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Session {} Packet", self.action)?;

        let component = &self.component;

        if let Some(player) = &self.session.player {
            writeln!(
                f,
                "Info: (Name: {}, ID: {}, SID: {})",
                &player.display_name, &player.id, &self.session.id
            )?;
        } else {
            writeln!(f, "Info: ( SID: {})", &self.session.id)?;
        }

        let minified = matches!(
            component,
            Components::Authentication(components::Authentication::ListUserEntitlements2)
                | Components::Util(components::Util::FetchClientConfig)
                | Components::Util(components::Util::UserSettingsLoadAll)
        );

        PacketDebug {
            packet: self.packet,
            component,
            minified,
        }
        .fmt(f)
    }
}

/// Encodes the session details for the provided session using
/// the provided writer
///
/// `session` The session to encode
/// `writer`  The writer to encode with
fn encode_session(session: &Session, writer: &mut TdfWriter) {
    session.net.tag_groups(b"ADDR", writer);
    writer.tag_str(b"BPS", "ea-sjc");
    writer.tag_str_empty(b"CTY");
    writer.tag_var_int_list_empty(b"CVAR");
    {
        writer.tag_map_start(b"DMAP", TdfType::VarInt, TdfType::VarInt, 1);
        writer.write_u32(0x70001);
        writer.write_u16(0x409a);
    }
    writer.tag_u16(b"HWFG", session.net.hardware_flags);
    {
        // Ping latency to the Quality of service servers
        writer.tag_list_start(b"PSLM", TdfType::VarInt, 1);
        0xfff0fff.encode(writer);
    }
    writer.tag_value(b"QDAT", &session.net.qos);
    writer.tag_u8(b"UATT", 0);
    if let Some(game_id) = &session.game {
        writer.tag_list_start(b"ULST", TdfType::Triple, 1);
        (4, 1, *game_id).encode(writer);
    }
    writer.tag_group_end();
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

impl Encodable for SessionUpdate<'_> {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_group(b"DATA");
        encode_session(self.session, writer);

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

/// Session update for ourselves
struct SetSession<'a> {
    /// The player ID the update is for
    player_id: PlayerID,
    /// The session this update is for
    session: &'a Session,
}

impl Encodable for SetSession<'_> {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_group(b"DATA");
        encode_session(self.session, writer);
        writer.tag_u32(b"USID", self.player_id);
    }
}
