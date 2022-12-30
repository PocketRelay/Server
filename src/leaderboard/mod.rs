//! Module for leaderboard related logic

use self::models::*;
use crate::{
    state::GlobalState,
    utils::{
        parsing::{parse_player_character, parse_player_class},
        types::PlayerID,
    },
};
use database::{DatabaseConnection, DbResult, Player};
use tokio::{sync::RwLock, task::JoinSet, try_join};

pub mod models;

/// Structure for storing the leaderboard values on the global
/// state.
#[derive(Default)]
pub struct Leaderboard {
    /// Leaderboard entity group for n7 ratings
    n7_group: RwLock<LeaderboardEntityGroup>,
    /// Leaderboard entity group for challenge points
    cp_group: RwLock<LeaderboardEntityGroup>,
}

/// Different query types for querying the leaderboards
/// in different ways
pub enum LeaderboardQuery {
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

impl Leaderboard {
    /// Retrieves the lock to the leaderboard entity group for the
    /// provided leaderboard type
    ///
    /// `ty` The leaderboard type
    fn get_type_lock(&self, ty: &LeaderboardType) -> &RwLock<LeaderboardEntityGroup> {
        match ty {
            LeaderboardType::N7Rating => &self.n7_group,
            LeaderboardType::ChallengePoints => &self.cp_group,
        }
    }

    /// Updates the provided leaderboard type. If the contents are
    /// expired then they are computed again. Returns a cloned list of
    /// entires matching the provided query or None if the query was not
    /// valid
    ///
    /// `ty` The leaderboard type
    pub async fn get(
        &self,
        ty: LeaderboardType,
        query: LeaderboardQuery,
    ) -> DbResult<Option<(Vec<LeaderboardEntry>, bool)>> {
        let read_lock = self.get_type_lock(&ty);
        // Check the cached value to see if its valid
        {
            let entity = &*read_lock.read().await;
            if entity.is_valid() {
                return Ok(Self::resolve_query(entity, query));
            }
        }

        let ranking = self.compute_rankings(&ty).await?;
        let entity = &mut *self.get_type_lock(&ty).write().await;
        entity.update(ranking);
        Ok(Self::resolve_query(entity, query))
    }
    /// Updates the provided leaderboard type. If the contents are
    /// expired then they are computed again. Returns the total number
    /// of entities present in the leaderboard type
    ///
    /// `ty` The leaderboard type
    pub async fn get_size(&self, ty: LeaderboardType) -> DbResult<usize> {
        let read_lock = self.get_type_lock(&ty);
        // Check the cached value to see if its valid
        {
            let entity = &*read_lock.read().await;
            if entity.is_valid() {
                return Ok(entity.values.len());
            }
        }

        let ranking = self.compute_rankings(&ty).await?;
        let entity = &mut *self.get_type_lock(&ty).write().await;
        entity.update(ranking);
        Ok(entity.values.len())
    }

    /// Resolves the query based on the provided entity group
    /// cloning any values that are needed returning a list of
    /// entires and a boolean for whether there are more entries
    /// after the current query.
    ///
    /// `group` The group to resolve with
    /// `query` The query to resolve
    fn resolve_query(
        group: &LeaderboardEntityGroup,
        query: LeaderboardQuery,
    ) -> Option<(Vec<LeaderboardEntry>, bool)> {
        let values = &group.values;
        let values_len = values.len();
        match query {
            LeaderboardQuery::Normal { start, count } => {
                // The index to stop at
                let end_index = count.min(values_len);

                values
                    .get(start..end_index)
                    .map(|value| (value.to_vec(), values_len > end_index))
            }
            LeaderboardQuery::Centered { player_id, count } => {
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
                values
                    .get(start_index..end_index)
                    .map(|value| (value.to_vec(), values_len > end_index))
            }
            LeaderboardQuery::Filtered { player_id } => {
                let player_entry = values
                    .iter()
                    .find(|value| value.player_id == player_id)
                    .cloned()?;

                Some((vec![player_entry], false))
            }
        }
    }

    /// Computes the ranking values for the provided `ty` this consists of
    /// streaming the values from the database in chunks of 20, processing the
    /// chunks converting them into entries then sorting the entries based
    /// on their value.
    ///
    /// `ty` The leaderboard type
    async fn compute_rankings(&self, ty: &LeaderboardType) -> DbResult<Vec<LeaderboardEntry>> {
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
            let mut join_set = JoinSet::new();
            match ty {
                LeaderboardType::N7Rating => {
                    for player in players {
                        join_set.spawn(Self::compute_n7_player(db, player));
                    }
                }
                LeaderboardType::ChallengePoints => {
                    for player in players {
                        join_set.spawn(Self::compute_cp_player(db, player));
                    }
                }
            }

            // Await computed results
            while let Some(value) = join_set.join_next().await {
                if let Ok(Ok(value)) = value {
                    values.push(value)
                }
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

    /// Computes the N7 rating for the provided player converting the player
    /// into a leaderboard entry where the value is the N7 rating of the player
    ///
    /// `db`     The database connection
    /// `player` The player to compute
    async fn compute_n7_player(
        db: &DatabaseConnection,
        player: Player,
    ) -> DbResult<LeaderboardEntry> {
        let mut total_promotions = 0;
        let mut total_level: u32 = 0;
        let (classes, characters) = try_join!(player.get_classes(db), player.get_characters(db),)?;

        let classes: Vec<_> = classes
            .into_iter()
            .filter_map(|value| parse_player_class(value.value))
            .collect();

        let characters: Vec<_> = characters
            .into_iter()
            .filter_map(|value| parse_player_character(value.value))
            .collect();

        for class in classes {
            // Classes are active if atleast one character from the class is deployed
            let is_active = characters
                .iter()
                .any(|char| char.kit_name.contains(&class.name) && char.deployed);
            if is_active {
                total_level += class.level as u32;
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

    async fn compute_cp_player(
        db: &DatabaseConnection,
        player: Player,
    ) -> DbResult<LeaderboardEntry> {
        let value = player.get_challenge_points(db).await.unwrap_or(0);
        Ok(LeaderboardEntry {
            player_id: player.id,
            player_name: player.display_name,
            // Rank is not computed yet at this stage
            rank: 0,
            value,
        })
    }
}
