//! Module for leaderboard related logic

use self::models::{LeaderboardEntityGroup, LeaderboardEntry, LeaderboardType};
use crate::state::GlobalState;
use database::{DatabaseConnection, DbResult, Player};
use tokio::sync::RwLock;

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

impl Leaderboard {
    /// Retrieves the lock to the leaderboard entity group for the
    /// provided leaderboard type
    ///
    /// `ty` The leaderboard type
    pub fn get_type_lock(&self, ty: &LeaderboardType) -> &RwLock<LeaderboardEntityGroup> {
        match ty {
            LeaderboardType::N7Rating => &self.n7_group,
            LeaderboardType::ChallengePoints => &self.cp_group,
        }
    }

    /// Updates the provided leaderboard type. If the contents are
    /// expired then they are computed again. Returns the total number
    /// of entities present in the leaderboard type and the lock used
    /// to access the entity
    ///
    /// `ty` The leaderboard type
    pub async fn get(
        &self,
        ty: LeaderboardType,
    ) -> DbResult<(usize, &RwLock<LeaderboardEntityGroup>)> {
        let read_lock = self.get_type_lock(&ty);
        // Check the cached value to see if its valid
        {
            let entity = &*read_lock.read().await;
            if entity.is_valid() {
                return Ok((entity.values.len(), read_lock));
            }
        }

        let ranking = self.compute_rankings(&ty).await?;
        let count = ranking.len();
        let entity = &mut *self.get_type_lock(&ty).write().await;
        entity.update(ranking);
        Ok((count, read_lock))
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
            match ty {
                LeaderboardType::N7Rating => {
                    Self::compute_n7_players(db, players, &mut values).await?
                }
                LeaderboardType::ChallengePoints => Self::compute_cp_players(players, &mut values),
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

    /// Computes the N7 ratings for all the provided players converting them
    /// into leaderboard entries with the player n7 rating as the value
    ///
    /// `db`      The database connection
    /// `players` The players to convert
    /// `output`  The output to append the entries to
    async fn compute_n7_players(
        db: &DatabaseConnection,
        players: Vec<Player>,
        output: &mut Vec<LeaderboardEntry>,
    ) -> DbResult<()> {
        let futures = players
            .into_iter()
            .map(|player| Self::compute_n7_player(db, player))
            .collect::<Vec<_>>();
        let results = futures_util::future::try_join_all(futures).await?;
        output.extend(results);
        Ok(())
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
        let (classes, characters) = player.collect_relations_partial(db).await?;
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

    /// Computes the challenge points for all the provided players converting
    /// them into leaderboard entries with the player challenge points as the value
    ///
    /// `players` The players to convert
    /// `output`  The output to append the entries to
    fn compute_cp_players(players: Vec<Player>, output: &mut Vec<LeaderboardEntry>) {
        for player in players {
            let value = player.get_challenge_points().unwrap_or(0);
            output.push(LeaderboardEntry {
                player_id: player.id,
                player_name: player.display_name,
                // Rank is not computed yet at this stage
                rank: 0,
                value,
            })
        }
    }
}
