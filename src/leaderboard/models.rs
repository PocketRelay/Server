use crate::utils::types::PlayerID;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    time::{Duration, SystemTime},
};

/// Structure for an entry in a leaderboard group
#[derive(Serialize, Deserialize)]
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
pub struct LeaderboardGroup {
    /// The values stored in this entity group
    pub values: Vec<LeaderboardEntry>,
    /// The time at which this entity group will become expired
    pub expires: SystemTime,
}

impl Default for LeaderboardGroup {
    fn default() -> Self {
        Self {
            values: Vec::with_capacity(0),
            expires: SystemTime::now(),
        }
    }
}

/// Different query types for querying the leaderboards
/// in different ways
pub enum LQuery {
    /// Normal query
    Normal {
        /// Offset amount to start at
        start: usize,
        /// Number of items to retrieve
        count: usize,
    },
    /// Query where the center is a specific player
    Centered {
        /// The ID of the player to center
        player_id: PlayerID,
        /// The number of players to query
        count: usize,
    },
    /// Returning a leaderboard filtered for
    /// a specific player
    Filtered {
        /// The ID of the player to get
        player_id: PlayerID,
    },
}

/// Result of a query from the leaderboard contains borrowed values
/// from the leaderboard group
pub enum LResult<'a> {
    // Query resulted in nothing being found
    Empty,
    // Query resulted in a single item response
    One(&'a LeaderboardEntry),
    // Query resulted in a many item response
    Many(&'a [LeaderboardEntry], bool),
}

impl LeaderboardGroup {
    /// Leaderboard contents are cached for 1 hour
    const LIFETIME: Duration = Duration::from_secs(60 * 60);

    /// Checks whether this group is expired
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now();
        now.ge(&self.expires)
    }

    /// Updates the stored values for this group and sets a new
    /// expiry time for the value
    pub fn update(&mut self, values: Vec<LeaderboardEntry>) {
        self.expires = SystemTime::now() + Self::LIFETIME;
        self.values = values;
    }

    /// Resolves the provided query on this entity group returning the LResult if it
    /// was able be resolved or None if it was unable to resolve
    ///
    /// `query` The query to resolve
    pub fn resolve(&self, query: LQuery) -> LResult {
        let values = &self.values;
        let values_len = values.len();
        match query {
            LQuery::Normal { start, count } => {
                // The index to stop at
                let end_index = count.min(values_len);

                values
                    .get(start..end_index)
                    .map(|value| LResult::Many(value, values_len > end_index))
            }
            LQuery::Centered { player_id, count } => {
                // The number of items before the center index
                let before = if count % 2 == 0 {
                    count / 2 + 1
                } else {
                    count / 2
                };
                // The number of items after the center index
                let after = count / 2;

                // The index of the centered player
                let player_index =
                    match values.iter().position(|value| value.player_id == player_id) {
                        Some(value) => value,
                        None => return LResult::Empty,
                    };

                // The index of the first item
                let start_index = player_index - before.min(player_index);
                // The index of the last item
                let end_index = (player_index + after).min(values_len);
                values
                    .get(start_index..end_index)
                    .map(|value| LResult::Many(value, values_len > end_index))
            }
            LQuery::Filtered { player_id } => values
                .iter()
                .find(|value| value.player_id == player_id)
                .map(LResult::One),
        }
        .unwrap_or(LResult::Empty)
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

    /// Gets the leaderboard type from the value provided
    /// by a Mass Effect client this would be either N7Rating
    /// or ChallangePoints along with the locale which in this
    /// case is ignored
    ///
    /// `value` The value to parse from
    pub fn from_value(value: &str) -> Self {
        if value.starts_with("N7Rating") {
            Self::N7Rating
        } else {
            Self::ChallengePoints
        }
    }
}
