use serde::{Deserialize, Serialize};
use utils::types::PlayerID;

#[derive(Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub player_id: PlayerID,
    pub player_name: String,
    pub rank: u32,
    pub value: String,
}
