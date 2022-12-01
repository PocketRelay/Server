use serde::{Deserialize, Serialize};
use utils::types::PlayerID;

#[derive(Serialize, Deserialize, Clone)]
pub struct LeaderboardEntry {
    pub player_id: PlayerID,
    pub player_name: String,
    pub rank: usize,
    pub value: u32,
}
