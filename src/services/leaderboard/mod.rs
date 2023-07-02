use self::models::*;
use crate::{
    database::{
        entities::players,
        entities::{Player, PlayerData},
        DatabaseConnection, DbResult,
    },
    state::App,
    utils::parsing::{KitNameDeployed, PlayerClass},
};
use interlink::prelude::*;
use log::{debug, error};
use sea_orm::{EntityTrait, PaginatorTrait, QueryOrder};
use std::{collections::HashMap, sync::Arc, time::Instant};
use tokio::task::JoinSet;

pub mod models;

#[derive(Service)]
pub struct Leaderboard {
    /// Map between the group types and the actual leaderboard group content
    groups: HashMap<LeaderboardType, GroupState>,
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

/// Message for requesting access to a leaderborad
/// of the specific leaderboard type
#[derive(Message)]
#[msg(rtype = "Arc<LeaderboardGroup>")]
pub struct QueryMessage(pub LeaderboardType);

impl Handler<QueryMessage> for Leaderboard {
    type Response = Fr<QueryMessage>;

    fn handle(&mut self, msg: QueryMessage, ctx: &mut ServiceContext<Self>) -> Self::Response {
        let ty = msg.0;

        // If the group already exists and is not expired we can respond with it
        if let Some(group) = self.groups.get_mut(&ty) {
            let inner = &group.group;

            // Response with current values if the group isn't expired or is computing
            if group.computing || !inner.is_expired() {
                // Value is not expired respond immediately
                return Fr::ready(inner.clone());
            }

            // Mark the group as currently being computed
            group.computing = true;
        } else {
            // Create dummy empty group to hand out while computing
            let dummy = GroupState {
                computing: true,
                group: Arc::new(LeaderboardGroup::dummy()),
            };
            self.groups.insert(ty, dummy);
        }

        let link = ctx.link();

        Fr::new(Box::pin(async move {
            // Compute new leaderboard values
            let values = Self::compute(&ty).await;
            let group = Arc::new(LeaderboardGroup::new(values));

            // Store the group and respond to the request
            let _ = link.do_send(SetGroupMessage {
                group: group.clone(),
                ty,
            });

            group
        }))
    }
}

/// Message used internally to update group state with
/// a new group value once a leaderboard has been
/// computed
#[derive(Message)]
struct SetGroupMessage {
    /// The leaderboard type to set
    ty: LeaderboardType,
    /// The new leaderboard value
    group: Arc<LeaderboardGroup>,
}

impl Handler<SetGroupMessage> for Leaderboard {
    type Response = ();

    fn handle(&mut self, msg: SetGroupMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        self.groups.insert(
            msg.ty,
            GroupState {
                computing: false,
                group: msg.group,
            },
        );
    }
}

impl Leaderboard {
    /// Starts a new leaderboard service
    pub fn start() -> Link<Leaderboard> {
        let this = Leaderboard {
            groups: Default::default(),
        };
        this.start()
    }

    /// Computes the ranking values for the provided `ty` this consists of
    /// streaming the values from the database in chunks of 20, processing the
    /// chunks converting them into entries then sorting the entries based
    /// on their value.
    ///
    /// `ty` The leaderboard type
    async fn compute(ty: &LeaderboardType) -> Box<[LeaderboardEntry]> {
        let start_time = Instant::now();

        // The amount of players to process in each database request
        const BATCH_COUNT: u64 = 20;

        let db = App::database();

        let mut values: Vec<LeaderboardEntry> = Vec::new();

        let mut join_set = JoinSet::new();

        let mut paginator = players::Entity::find()
            .order_by_asc(players::Column::Id)
            .paginate(db, BATCH_COUNT);

        // Function pointer to the computing function for the desired type
        let fun: fn(&'static DatabaseConnection, Player) -> Lf = match ty {
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
                join_set.spawn(fun(db, player));
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
fn compute_n7_player(db: &'static DatabaseConnection, player: Player) -> Lf {
    Box::pin(async move {
        let mut total_promotions: u32 = 0;
        let mut total_level: u32 = 0;

        let data: Vec<PlayerData> = PlayerData::all(db, player.id).await?;

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
fn compute_cp_player(db: &'static DatabaseConnection, player: Player) -> Lf {
    Box::pin(async move {
        let value = PlayerData::get_challenge_points(db, player.id)
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
