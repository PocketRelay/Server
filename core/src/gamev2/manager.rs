use std::{
    collections::{HashMap, VecDeque},
    sync::atomic::AtomicU32,
    time::SystemTime,
};

use tokio::sync::RwLock;

use crate::{blaze::session::SessionArc, game::rules::RuleSet};

use super::game::Game;

/// Structure for managing games and the matchmaking queue
pub struct GameManager {
    /// Map of Game IDs to the actual games.
    games: RwLock<HashMap<u32, Game>>,
    /// Queue of players wanting to join games
    queue: RwLock<VecDeque<QueueEntry>>,
    /// ID for the next game to create
    id: AtomicU32,
}

/// Structure for a entry in the matchmaking queue
struct QueueEntry {
    /// The session that is waiting in the queue
    session: SessionArc,
    /// The rules that games must meet for this
    /// queue entry to join.
    rules: RuleSet,
    /// The time that the queue entry was created at
    time: SystemTime,
}
