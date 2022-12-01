use std::time::{Duration, SystemTime};

use tokio::sync::RwLock;

use crate::state::GlobalState;

use super::models::LeaderboardEntry;
use database::{DatabaseConnection, DbResult, Player};

#[derive(Default)]
pub struct Leaderboard {
    pub n7_group: RwLock<LeaderboardEntityGroup>,
    pub cp_group: RwLock<LeaderboardEntityGroup>,
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
    const LIFETIME: Duration = Duration::from_secs(60 * 60 * 1);

    /// Creates a new entity group from the provided values that expires
    /// after the provided duration is elapsed
    ///
    /// `values`   The leaderboard values
    /// `duration` The duration the entity will be used for
    pub fn new(values: Vec<LeaderboardEntry>, duration: Duration) -> Self {
        let expires = SystemTime::now() + duration;
        Self { values, expires }
    }

    pub fn is_valid(&self) -> bool {
        let now = SystemTime::now();
        now.lt(&self.expires)
    }

    pub fn update(&mut self, values: Vec<LeaderboardEntry>) {
        self.expires = SystemTime::now() + Self::LIFETIME;
        self.values = values;
    }
}

impl Leaderboard {
    /// Updates the stored N7 ratings leaderboard computing the rankings again if
    /// the expiry time has been reached. Returns the total number of entries
    /// in the leaderboard.
    pub async fn update_n7(&self) -> DbResult<usize> {
        {
            let existing = &*self.n7_group.read().await;
            if existing.is_valid() {
                return Ok(existing.values.len());
            }
        }
        let ratings = self.compute_n7_rankings().await?;
        let count = ratings.len();
        let existing = &mut *self.n7_group.write().await;
        existing.update(ratings);
        Ok(count)
    }

    pub async fn update_cp(&self) -> DbResult<usize> {
        {
            let existing = &*self.cp_group.read().await;
            if existing.is_valid() {
                return Ok(existing.values.len());
            }
        }
        let ratings = self.compute_cp_ratings().await?;
        let count = ratings.len();
        let existing = &mut *self.cp_group.write().await;
        existing.update(ratings);
        Ok(count)
    }

    async fn compute_cp_ratings(&self) -> DbResult<Vec<LeaderboardEntry>> {
        // The amount of players to process in each database request
        const BATCH_COUNT: u64 = 20;
        let db = GlobalState::database();

        let mut offset = 0;
        let mut values: Vec<LeaderboardEntry> = Vec::new();
        loop {
            let (players, more) = Player::all(db, offset, BATCH_COUNT).await?;
            if players.is_empty() {
                break;
            }

            for player in players {
                let value = player.get_challenge_points().unwrap_or(0);
                values.push(LeaderboardEntry {
                    player_id: player.id,
                    player_name: player.display_name,
                    // Rank is not computed yet at this stage
                    rank: 0,
                    value,
                })
            }

            if !more {
                break;
            }

            offset += BATCH_COUNT;
        }
        // Sort the values based on their value
        values.sort_by(|a, b| a.value.cmp(&b.value).reverse());

        // Apply the new rank order
        let mut rank = 1;
        for value in values.iter_mut() {
            value.rank = rank;
            rank += 1;
        }

        Ok(values)
    }

    async fn compute_n7_rankings(&self) -> DbResult<Vec<LeaderboardEntry>> {
        // The amount of players to process in each database request
        const BATCH_COUNT: u64 = 20;

        let db = GlobalState::database();

        let mut offset = 0;
        let mut values: Vec<LeaderboardEntry> = Vec::new();
        loop {
            let (players, more) = Player::all(db, offset, BATCH_COUNT).await?;
            if players.is_empty() {
                break;
            }
            let futures = players
                .into_iter()
                .map(|player| Self::compute_player_n7(db, player))
                .collect::<Vec<_>>();
            let results = futures::future::try_join_all(futures).await?;
            values.extend(results);
            if !more {
                break;
            }

            offset += BATCH_COUNT;
        }
        // Sort the values based on their value
        values.sort_by(|a, b| a.value.cmp(&b.value).reverse());
        // Apply the new rank order
        let mut rank = 1;
        for value in values.iter_mut() {
            value.rank = rank;
            rank += 1;
        }
        Ok(values)
    }

    async fn compute_player_n7(
        db: &DatabaseConnection,
        player: Player,
    ) -> DbResult<LeaderboardEntry> {
        let mut total_promotions = 0;
        let mut total_level: u32 = 0;
        let (classes, characters) = player.collect_relations_partial(db).await?;
        for class in classes {
            // Classes are active if atleast one character from the class is deployed
            let is_active = characters
                .iter()
                .any(|char| char.kit_name.contains(&class.name) && char.deployed);
            if is_active {
                total_level += class.level;
            }
            total_promotions += class.promotions;
        }
        // 30 -> 20 from leveling class + 10 bonus for promoting
        let rating = total_promotions * 30 + total_level;
        Ok(LeaderboardEntry {
            player_id: player.id,
            player_name: player.display_name,
            // Rank is not computed yet at this stage
            rank: 0,
            value: rating,
        })
    }
}
