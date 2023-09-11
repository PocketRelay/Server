use self::models::*;
use crate::{
    database::{
        entities::players,
        entities::{Player, PlayerData},
        DatabaseConnection, DbResult,
    },
    utils::parsing::{KitNameDeployed, PlayerClass},
};
use interlink::prelude::*;
use log::{debug, error};
use sea_orm::{EntityTrait, PaginatorTrait, QueryOrder};
use std::{collections::HashMap, sync::Arc, time::Instant};
use tokio::{sync::RwLock, task::JoinSet};

pub mod models;

pub struct Leaderboard {
    /// Map between the group types and the actual leaderboard group content
    groups: RwLock<HashMap<LeaderboardType, GroupState>>,
}

/// Extra state wrapper around a leaderboard group which
/// holds the state of whether the group is being actively
/// recomputed
struct GroupState {
    /// Whether the group is being computed
    computing: bool,
    /// The underlying group
    group: Arc<LeaderboardGroup>,
}

impl Leaderboard {
    /// Starts a new leaderboard service
    pub fn new() -> Leaderboard {
        Leaderboard {
            groups: Default::default(),
        }
    }

    pub async fn query(
        &self,
        ty: LeaderboardType,
        db: &DatabaseConnection,
    ) -> Arc<LeaderboardGroup> {
        {
            let groups = &mut *self.groups.write().await;
            // If the group already exists and is not expired we can respond with it
            if let Some(group) = groups.get_mut(&ty) {
                let inner = &group.group;

                // Response with current values if the group isn't expired or is computing
                if group.computing || !inner.is_expired() {
                    // Value is not expired respond immediately
                    return inner.clone();
                }

                // Mark the group as currently being computed
                group.computing = true;
            } else {
                // Create dummy empty group to hand out while computing
                let dummy = GroupState {
                    computing: true,
                    group: Arc::new(LeaderboardGroup::dummy()),
                };
                groups.insert(ty, dummy);
            }
        }

        // Compute new leaderboard values
        let values = Self::compute(&ty, db).await;
        let group = Arc::new(LeaderboardGroup::new(values));

        // Store the updated group
        {
            let groups = &mut *self.groups.write().await;
            groups.insert(
                ty,
                GroupState {
                    computing: false,
                    group: group.clone(),
                },
            );
        }

        group
    }

    /// Computes the ranking values for the provided `ty` this consists of
    /// streaming the values from the database in chunks of 20, processing the
    /// chunks converting them into entries then sorting the entries based
    /// on their value.
    ///
    /// `ty` The leaderboard type
    async fn compute(ty: &LeaderboardType, db: &DatabaseConnection) -> Box<[LeaderboardEntry]> {
        let start_time = Instant::now();

        // The amount of players to process in each database request
        const BATCH_COUNT: u64 = 20;

        let mut values: Vec<LeaderboardEntry> = Vec::new();

        let mut join_set = JoinSet::new();

        let mut paginator = players::Entity::find()
            .order_by_asc(players::Column::Id)
            .paginate(db, BATCH_COUNT);

        // Function pointer to the computing function for the desired type
        let fun: fn(DatabaseConnection, Player) -> Lf = match ty {
            LeaderboardType::N7Rating => compute_n7_player,
            LeaderboardType::ChallengePoints => compute_cp_player,
        };

        loop {
            let players = match paginator.fetch_and_next().await {
                Ok(None) => break,
                Ok(Some(value)) => value,
                Err(err) => {
                    error!("Unable to load players for leaderboard: {:?}", err);
                    break;
                }
            };

            // Add the futures for all the players
            for player in players {
                join_set.spawn(fun(db.clone(), player));
            }

            // Await computed results
            while let Some(value) = join_set.join_next().await {
                if let Ok(Ok(value)) = value {
                    values.push(value)
                }
            }
        }

        // Sort the values based on their value
        values.sort_by(|a, b| b.value.cmp(&a.value));

        // Apply the new rank order to the rank values
        let mut rank = 1;
        for value in &mut values {
            value.rank = rank;
            rank += 1;
        }

        debug!("Computed leaderboard took: {:.2?}", start_time.elapsed());

        values.into_boxed_slice()
    }
}

type Lf = BoxFuture<'static, DbResult<LeaderboardEntry>>;

/// Computes a ranking for the provided player based on the N7 ranking
/// of that player.
///
/// `db`     The database connection
/// `player` The player to rank
fn compute_n7_player(db: DatabaseConnection, player: Player) -> Lf {
    Box::pin(async move {
        let mut total_promotions: u32 = 0;
        let mut total_level: u32 = 0;

        let data: Vec<PlayerData> = PlayerData::all(&db, player.id).await?;

        let mut classes: Vec<PlayerClass> = Vec::new();
        let mut characters: Vec<KitNameDeployed> = Vec::new();

        for datum in &data {
            if datum.key.starts_with("class") {
                if let Some(value) = PlayerClass::parse(&datum.value) {
                    classes.push(value);
                }
            } else if datum.key.starts_with("char") {
                if let Some(value) = KitNameDeployed::parse(&datum.value) {
                    characters.push(value);
                }
            }
        }

        for class in classes {
            // Classes are active if atleast one character from the class is deployed
            let is_active = characters
                .iter()
                .any(|char| char.kit_name.contains(class.name) && char.deployed);
            if is_active {
                total_level += class.level as u32;
            }
            total_promotions += class.promotions;
        }

        // 30 -> 20 from leveling class + 10 bonus for promoting
        let rating: u32 = total_promotions * 30 + total_level;
        Ok(LeaderboardEntry {
            player_id: player.id,
            player_name: player.display_name.into_boxed_str(),
            // Rank is not computed yet at this stage
            rank: 0,
            value: rating,
        })
    })
}

/// Computes a ranking for the provided player based on the number of
/// challenge points the player has
///
/// `db`     The database connection
/// `player` The player to rank
fn compute_cp_player(db: DatabaseConnection, player: Player) -> Lf {
    Box::pin(async move {
        let value = PlayerData::get_challenge_points(&db, player.id)
            .await
            .unwrap_or(0);
        Ok(LeaderboardEntry {
            player_id: player.id,
            player_name: player.display_name.into_boxed_str(),
            // Rank is not computed yet at this stage
            rank: 0,
            value,
        })
    })
}
