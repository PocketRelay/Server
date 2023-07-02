use crate::utils::types::PlayerID;
use serde::Serialize;
use std::{
    fmt::Display,
    time::{Duration, SystemTime},
};

/// Structure for an entry in a leaderboard group
#[derive(Serialize)]
pub struct LeaderboardEntry {
    /// The ID of the player this entry is for
    pub player_id: PlayerID,
    /// The name of the player this entry is for
    pub player_name: Box<str>,
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
    pub values: Box<[LeaderboardEntry]>,
    /// The time at which this entity group will become expired
    pub expires: SystemTime,
}

impl LeaderboardGroup {
    /// Leaderboard contents are cached for 1 hour
    const LIFETIME: Duration = Duration::from_secs(60 * 60);

    /// Creates a new leaderboard group which has an expiry time set
    /// to the LIFETIME and uses the provided values
    pub fn new(values: Box<[LeaderboardEntry]>) -> Self {
        let expires = SystemTime::now() + Self::LIFETIME;
        Self { expires, values }
    }

    /// Creates a dummy leaderboard group which has no values and
    /// is already considered to be expired. Used to hand out
    /// a value while computed to prevent mulitple computes happening
    pub fn dummy() -> Self {
        Self {
            expires: SystemTime::UNIX_EPOCH,
            values: Box::new([]),
        }
    }

    /// Checks whether this group is expired
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now();
        now.ge(&self.expires)
    }

    /// Gets a normal collection of leaderboard entries at the start offset of the
    /// provided count. Will return the slice of entires as well as whether there are
    /// more entries after the desired offset
    ///
    /// `start` The start offset index
    /// `count` The number of leaderboard entries
    pub fn get_normal(&self, start: usize, count: usize) -> Option<(&[LeaderboardEntry], bool)> {
        let values = &self.values;
        let values_len = values.len();

        // The index to stop at
        let end_index = (start + count).min(values_len);

        values
            .get(start..end_index)
            .map(|value| (value, values_len > end_index))
    }

    /// Gets a leaderboard entry for the provided player ID if one is present
    ///
    /// `player_id` The ID of the player to find the entry for
    pub fn get_entry(&self, player_id: PlayerID) -> Option<&LeaderboardEntry> {
        let values = &self.values;
        values.iter().find(|value| value.player_id == player_id)
    }

    /// Gets a collection of leaderboard entries centered on the provided player with
    /// half `count` items before and after if possible.
    ///
    /// `player_id` The ID of the player to center on
    /// `count`     The total number of players to center on
    pub fn get_centered(&self, player_id: PlayerID, count: usize) -> Option<&[LeaderboardEntry]> {
        let values = &self.values;
        let values_len = values.len();
        // The number of items before the center index
        let before = if count % 2 == 0 {
            count / 2 + 1
        } else {
            count / 2
        };
        // The number of items after the center index
        let after = count / 2;

        // The index of the centered player
        let player_index = values
            .iter()
            .position(|value| value.player_id == player_id)?;

        // The index of the first item
        let start_index = player_index - before.min(player_index);
        // The index of the last item
        let end_index = (player_index + after).min(values_len);

        values.get(start_index..end_index)
    }
}

/// Type of leaderboard entity
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq)]
pub enum LeaderboardType {
    /// Leaderboard based on the player N7 ratings
    N7Rating,
    /// Leaderboard based on the player challenge point number
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
