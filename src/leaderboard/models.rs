use crate::utils::types::PlayerID;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    time::{Duration, SystemTime},
};

/// Structure for an entry in a leaderboard group
#[derive(Serialize, Deserialize, Clone)]
pub struct LeaderboardEntry {
    /// The ID of the player this entry is for
    pub player_id: PlayerID,
    /// The name of the player this entry is for
    pub player_name: String,
    /// The ranking of this entry (Position in the leaderboard)
    pub rank: usize,
    /// The value this ranking is based on
    pub value: u32,
}

/// Structure for a group of leaderboard entities ranked based
/// on a certain value the expires indicates when the value will
/// no longer be considered valid
pub struct LeaderboardEntityGroup {
    /// The values stored in this entity group
    pub values: Vec<LeaderboardEntry>,
    /// The time at which this entity group will become expired
    pub expires: SystemTime,
}

impl Default for LeaderboardEntityGroup {
    fn default() -> Self {
        Self {
            values: Vec::with_capacity(0),
            expires: SystemTime::now(),
        }
    }
}

impl LeaderboardEntityGroup {
    /// Leaderboard contents are cached for 1 hour
    const LIFETIME: Duration = Duration::from_secs(60 * 60);

    pub fn is_valid(&self) -> bool {
        let now = SystemTime::now();
        now.lt(&self.expires)
    }

    pub fn update(&mut self, values: Vec<LeaderboardEntry>) {
        self.expires = SystemTime::now() + Self::LIFETIME;
        self.values = values;
    }
}

/// Type of leaderboard entity
pub enum LeaderboardType {
    N7Rating,
    ChallengePoints,
}

impl Display for LeaderboardType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::N7Rating => "N7 Rating",
            Self::ChallengePoints => "Challenge Points",
        })
    }
}

impl LeaderboardType {
    /// Attempts to parse the leaderboard type from the provided value
    ///
    /// `value` The value to attempt to parse from
    pub fn try_parse(value: &str) -> Option<LeaderboardType> {
        if value.eq_ignore_ascii_case("n7") {
            Some(LeaderboardType::N7Rating)
        } else if value.eq_ignore_ascii_case("cp") {
            Some(LeaderboardType::ChallengePoints)
        } else {
            None
        }
    }
}

impl From<String> for LeaderboardType {
    fn from(value: String) -> Self {
        if value.starts_with("N7Rating") {
            Self::N7Rating
        } else {
            Self::ChallengePoints
        }
    }
}
