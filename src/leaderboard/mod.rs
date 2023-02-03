//! Module for leaderboard related logic

use self::models::*;
use crate::{
    state::GlobalState,
    utils::parsing::{parse_player_character, parse_player_class},
};
use database::{DatabaseConnection, DbResult, Player};
use log::error;
use std::{future::Future, pin::Pin};
use tokio::{sync::RwLock, task::JoinSet, try_join};

pub mod models;

/// Structure for storing the leaderboard values on the global
/// state.
#[derive(Default)]
pub struct Leaderboard {
    /// Leaderboard entity group for n7 ratings
    n7_group: RwLock<LeaderboardGroup>,
    /// Leaderboard entity group for challenge points
    cp_group: RwLock<LeaderboardGroup>,
}

impl Leaderboard {
    /// Retrieves a lock to the leaderboard group of the provided type.
    /// Checks if the stored group is expired and if it is the new ranking
    /// values will be computed before returning the lock.
    ///
    /// `ty` The leaderboard type
    pub async fn get(&self, ty: LeaderboardType) -> &RwLock<LeaderboardGroup> {
        let lock = match &ty {
            LeaderboardType::N7Rating => &self.n7_group,
            LeaderboardType::ChallengePoints => &self.cp_group,
        };

        // Check if the data is expired
        let is_expired = {
            let group = lock.read().await;
            group.is_expired()
        };

        // if expired compute the data again
        if is_expired {
            // Hold the write lock while compute so others don't try and compute
            let group = &mut *lock.write().await;

            // Compute the rankings and update the group
            let ranking = self.compute(ty).await;
            group.update(ranking);
        }

        lock
    }

    /// Computes the ranking values for the provided `ty` this consists of
    /// streaming the values from the database in chunks of 20, processing the
    /// chunks converting them into entries then sorting the entries based
    /// on their value.
    ///
    /// `ty` The leaderboard type
    async fn compute(&self, ty: LeaderboardType) -> Vec<LeaderboardEntry> {
        // The amount of players to process in each database request
        const BATCH_COUNT: u64 = 20;

        let db = GlobalState::database();

        // The current database batch offset position
        let mut offset = 0;
        let mut values: Vec<LeaderboardEntry> = Vec::new();

        // Decide the ranking function to use based on the type
        let ranking_fn: Box<dyn Ranker> = match ty {
            LeaderboardType::N7Rating => Box::new(compute_n7_player),
            LeaderboardType::ChallengePoints => Box::new(compute_cp_player),
        };

        let mut join_set = JoinSet::new();

        loop {
            let (players, more) = match Player::all(&db, offset, BATCH_COUNT).await {
                Ok((ref players, _)) if players.is_empty() => break,
                Ok(value) => value,
                Err(err) => {
                    error!("Unable to load players for leaderboard: {:?}", err);
                    break;
                }
            };

            // Add the futures for all the players
            for player in players {
                join_set.spawn(ranking_fn.compute_ranking(db.clone(), player));
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
        values.sort_by(|a, b| b.value.cmp(&a.value));

        // Apply the new rank order to the rank values
        let mut rank = 1;
        for value in &mut values {
            value.rank = rank;
            rank += 1;
        }

        values
    }
}

/// Type alias for pinned boxed futures that return a leaderboard entry inside DbResult
type RankerFut = Pin<Box<dyn Future<Output = DbResult<LeaderboardEntry>> + Send + 'static>>;

/// Trait implemented by things that can be used to return futures
trait Ranker: Send {
    /// Function for producing the future that on completion will result
    /// in the leaderboard entry value
    fn compute_ranking(&self, db: DatabaseConnection, player: Player) -> RankerFut;
}

/// Ranker implementaion for function types
///
/// ```
/// async fn test(db: &DatabaseConnection, player: Player) -> DbResult<LeaderboardEntry> {
///     /* Compute the ranking */
/// }
/// ```
impl<F, Fut> Ranker for F
where
    F: Fn(DatabaseConnection, Player) -> Fut + Send,
    Fut: Future<Output = DbResult<LeaderboardEntry>> + Send + 'static,
{
    fn compute_ranking(&self, db: DatabaseConnection, player: Player) -> RankerFut {
        Box::pin(self(db, player))
    }
}

/// Computes a ranking for the provided player based on the N7 ranking
/// of that player.
///
/// `db`     The database connection
/// `player` The player to rank
async fn compute_n7_player(db: DatabaseConnection, player: Player) -> DbResult<LeaderboardEntry> {
    let mut total_promotions = 0;
    let mut total_level: u32 = 0;
    let (classes, characters) = try_join!(player.get_classes(&db), player.get_characters(&db),)?;

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

/// Computes a ranking for the provided player based on the number of
/// challenge points the player has
///
/// `db`     The database connection
/// `player` The player to rank
async fn compute_cp_player(db: DatabaseConnection, player: Player) -> DbResult<LeaderboardEntry> {
    let value = player.get_challenge_points(&db).await.unwrap_or(0);
    Ok(LeaderboardEntry {
        player_id: player.id,
        player_name: player.display_name,
        // Rank is not computed yet at this stage
        rank: 0,
        value,
    })
}
