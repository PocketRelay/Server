use self::models::*;
use crate::database::entities::players;
use crate::database::{DatabaseConnection, DbResult, Player};
use crate::{
    state::GlobalState,
    utils::{
        parsing::{KitNameDeployed, PlayerClass},
        types::BoxFuture,
    },
};
use interlink::prelude::*;
use log::{debug, error};
use sea_orm::{EntityTrait, PaginatorTrait, QueryOrder};
use std::{collections::HashMap, future::Future, sync::Arc, time::SystemTime};
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
    async fn compute(ty: &LeaderboardType) -> Vec<LeaderboardEntry> {
        let start_time = SystemTime::now();

        // The amount of players to process in each database request
        const BATCH_COUNT: u64 = 20;

        let db = GlobalState::database();

        let mut values: Vec<LeaderboardEntry> = Vec::new();

        // Decide the ranking function to use based on the type
        let ranking: Box<dyn Ranker> = ty.into();

        let mut join_set = JoinSet::new();

        let mut paginator = players::Entity::find()
            .order_by_asc(players::Column::Id)
            .paginate(&db, BATCH_COUNT);

        loop {
            let players = match paginator.fetch_and_next().await {
                Ok(None) => break,
                Ok(Some(value)) => value,
                Err(err) => {
                    error!("Unable to load players for leaderboard: {:?}", err);
                    break;
                }
            };

            if players.is_empty() {
                break;
            }

            // Add the futures for all the players
            for player in players {
                join_set.spawn(ranking.compute_ranking(db.clone(), player));
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

        let end_time = SystemTime::now();
        if let Ok(duration) = end_time.duration_since(start_time) {
            debug!("Computed leaderboard took: {:.2?}", duration)
        }

        values
    }
}

impl From<&LeaderboardType> for Box<dyn Ranker> {
    fn from(value: &LeaderboardType) -> Self {
        match value {
            LeaderboardType::N7Rating => Box::new(compute_n7_player),
            LeaderboardType::ChallengePoints => Box::new(compute_cp_player),
        }
    }
}

/// Type alias for pinned boxed futures that return a leaderboard entry inside DbResult
type RankerFut = BoxFuture<'static, DbResult<LeaderboardEntry>>;

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

    let data = Player::all_data(player.id, &db).await?;

    let mut classes = Vec::new();
    let mut characters = Vec::new();

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
