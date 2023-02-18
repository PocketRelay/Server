//! Sessions are client connections to the main server with associated
//! data such as player data for when they become authenticated and
//! networking data.
use super::models::errors::{ServerError, ServerResult};
use super::router;
use crate::utils::types::PlayerID;
use crate::{
    services::game::{player::GamePlayer, RemovePlayerType},
    state::GlobalState,
    utils::{
        components::{self, Components, UserSessions},
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
    fn handle(
        &mut self,
        msg: PacketMessage,
        ctx: &mut ServiceContext<Self>,
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
        if let Ok(msg) = msg {
            self.handle_packet(ctx, msg);
        } else {
            ctx.stop();
        }
    }
}

impl ErrorHandler<io::Error> for Session {
    fn handle(&mut self, err: io::Error, ctx: &mut ServiceContext<Self>) -> ErrorAction {
        ErrorAction::Continue
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

    /// Handles processing a recieved packet from the `process` function.
    /// The buffer is flushed after routing is complete.
    ///
    /// `session`   The session to process the packet for
    /// `component` The component of the packet for routing
    /// `packet`    The packet itself
    fn handle_packet(&mut self, ctx: &mut ServiceContext<Self>, packet: Packet) {
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

    /// Sets the game details for the current session and updates
    /// the client with the new sesion details
    ///
    /// `game` The game the player has joined.
    /// `slot` The slot in the game the player is in.
    pub fn set_game(&mut self, game: Option<GameID>) {
        self.game = game;
        self.update_client();
    }

    /// Updates the networking information for this session making
    /// it a set and setting the ext and groups. Updating the client
    /// with the new session details
    ///
    /// `groups` The networking groups
    /// `ext`    The networking ext
    pub fn set_network_info(&mut self, groups: NetGroups, ext: QosNetworkData) {
        let net = &mut &mut self.net;
        net.qos = ext;
        net.groups = Some(groups);
        self.update_client();
    }

    /// Updates the hardware flag for this session and
    /// updates the client with the changes
    ///
    /// `value` The new hardware flag value
    pub fn set_hardware_flag(&mut self, value: u16) {
        self.net.hardware_flags = value;
        self.update_client();
    }

    /// Updates the data stored on the client so that it matches
    /// the data stored in this session
    fn update_client(&mut self) {
        let player_id = self.player.as_ref().map(|player| player.id).unwrap_or(1);
        let packet = Packet::notify(
            Components::UserSessions(UserSessions::SetSession),
            SetSession {
                player_id,
                session: self,
            },
        );
        self.push(packet);
    }

    pub fn push_details(&mut self, addr: SessionLink) {
        let player = match self.player.as_ref() {
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

        // Push the packets
        addr.push(a);
        addr.push(b);
    }

    /// Removes the session from any connected games and the
    /// matchmaking queue
    pub fn remove_games(&mut self) {
        let game = self.game.take();
        let services = GlobalState::services();
        if let Some(game_id) = game {
            services
                .game_manager
                .remove_player(game_id, RemovePlayerType::Session(self.id))
        } else {
            services.matchmaking.unqueue_session(self.id);
        }
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
