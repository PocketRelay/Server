//! This module contains the storage and additional data for sessions. Sessions
//! are data attached to streams that can be manipulated. Sessions are stored
//! behind Arc's and are cloned into Games and other resources. Sesssion must be
//! removed from all other structs in the release function.

use std::{collections::VecDeque, net::SocketAddr, sync::Arc, time::SystemTime};

use crate::{database::entities::players, game::GameArc, GlobalStateArc};
use log::debug;
use tokio::{
    net::TcpStream,
    sync::{mpsc, Mutex, RwLock},
};

use super::shared::NetData;

/// Structure for storing a client session. This includes the
/// network stream for the client along with global state and
/// other session state.
pub struct Session {
    /// Reference to the global state. In order to access
    /// the database and other shared functionality
    pub global: GlobalStateArc,

    /// Unique identifier for this session.
    pub id: u32,

    /// Underlying connection stream to client
    pub stream: Mutex<TcpStream>,
    /// The socket connection address of the client
    pub addr: SocketAddr,

    /// Additional data stored on this session.
    pub data: RwLock<SessionData>,

    /// Buffer for notify packets that need to be written
    /// and flushed.
    buffer: SessionBuffer,

    /// Extra information for this session to include in
    /// the debug messages.
    debug_state: RwLock<String>,
}

/// Type for session wrapped in Arc
pub type SessionArc = Arc<Session>;

impl Drop for Session {
    fn drop(&mut self) {
        debug!("Session dropped (SID: {})", self.id);
    }
}

/// Structure for buffering packet writes with flushing
/// functionality.
struct SessionBuffer {
    /// Queue of encoded packet bytes behind mutex for thread safety
    queue: Mutex<VecDeque<Vec<u8>>>,
    /// Sender for telling the session processor when the queue needs
    /// to be flushed.
    flush: mpsc::Sender<()>,
}

/// Structure for storing session data that is mutated often. This
/// data is placed behind a RwLock so it can be modified.
pub struct SessionData {
    /// If the session is authenticated it will have a linked
    /// player model from the database
    pub player: Option<players::Model>,

    /// Encoded location data. The format or values of this are not
    /// yet documented.
    pub location: u32,

    /// The system time that the last client ping was recieved at.
    /// Currently unused but should in future be used to timeout clients.
    pub last_ping: SystemTime,

    /// Networking information
    pub net: NetData,

    /// Hardware flag name might be incorrect usage is unknown
    pub hardware_flag: u16,

    // Appears to be some sort of client state. Needs further documentation
    pub state: u8,

    /// Matchmaking state if the player is matchmaking.
    pub matchmaking: bool,

    /// Game details if the player is in a game.
    pub game: Option<SessionGame>,
}

/// Structure for storing information about the game
/// which a session is connected to.
pub struct SessionGame {
    /// Reference to the game that the player is in.
    pub game: GameArc,
    /// The slot in the game which the player is in.
    pub slot: usize,
}
