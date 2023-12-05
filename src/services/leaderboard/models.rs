use crate::utils::types::PlayerID;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

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

    /// Checks whether there are more items after the provided offset and size
    pub fn has_more(&self, start: usize, count: usize) -> bool {
        let length = self.values.len();
        start + count < length
    }

    /// Gets a normal collection of leaderboard entries at the start offset of the
    /// provided count. Will return the slice of entires as well as whether there are
    /// more entries after the desired offset
    ///
    /// `start` The start offset index
    /// `count` The number of leaderboard entries
    pub fn get_normal(&self, start: usize, count: usize) -> Option<&[LeaderboardEntry]> {
        let end_index = (start + count).min(self.values.len());
        self.values.get(start..end_index)
    }

    /// Gets a leaderboard entry for the provided player ID if one is present
    ///
    /// `player_id` The ID of the player to find the entry for
    pub fn get_entry(&self, player_id: PlayerID) -> Option<&LeaderboardEntry> {
        self.values
            .iter()
            .find(|value| value.player_id == player_id)
    }

    pub fn get_filtered(&self, players: &[PlayerID]) -> Vec<&LeaderboardEntry> {
        self.values
            .iter()
            .filter(move |entry| players.contains(&entry.player_id))
            .collect()
    }

    /// Gets a collection of leaderboard entries centered on the provided player with
    /// half `count` items before and after if possible.
    ///
    /// `player_id` The ID of the player to center on
    /// `count`     The total number of players to center on
    pub fn get_centered(&self, player_id: PlayerID, count: usize) -> Option<&[LeaderboardEntry]> {
        if count == 0 {
            return None;
        }

        // The number of items before the center index
        let before = if count % 2 == 0 {
            (count / 2).saturating_add(1)
        } else {
            count / 2
        };

        // The number of items after the center index
        let after = count / 2;

        // The index of the centered player
        let player_index = self
            .values
            .iter()
            .position(|value| value.player_id == player_id)?;

        // The index of the first item
        let start_index = player_index.saturating_sub(before).min(player_index);
        // The index of the last item
        let end_index = player_index.saturating_add(after).min(self.values.len());

        self.values.get(start_index..end_index)
    }
}
