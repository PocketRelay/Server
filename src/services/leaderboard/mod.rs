//! Module for leaderboard related logic

use self::models::*;
use crate::{
    state::GlobalState,
    utils::{
        parsing::{parse_player_character, parse_player_class},
        types::BoxFuture,
    },
};
use database::{DatabaseConnection, DbResult, Player};
use interlink::prelude::*;
use log::error;
use std::{collections::HashMap, future::Future, sync::Arc};
use tokio::{sync::oneshot, task::JoinSet, try_join};

pub mod models;

#[derive(Default)]
struct Leaderboard {
    /// Map between the group types and the actual leaderboard group content
    groups: HashMap<LeaderboardType, Arc<LeaderboardGroup>>,
}

/// Request message for retrie
struct GetRequest {
    ty: LeaderboardType,
    tx: oneshot::Sender<Arc<LeaderboardGroup>>,
}

impl Message for GetRequest {
    type Response = ();
}

impl Service for Leaderboard {}

impl Handler<GetRequest> for Leaderboard {
    fn handle(&mut self, msg: GetRequest, ctx: &mut ServiceContext<Self>) {
        // If the group already exists and is not expired we can respond with it
        if let Some(group) = self.groups.get(&msg.ty) {
            if !group.is_expired() {
                // Value is not expire respond immediately
                msg.tx.send(group.clone()).ok();
                return;
            }
        }

        let link = ctx.link();
        link.do_wait(move |service, _| {
            Box::pin(async move {
                // Compute the leaderboard
                let values = service.compute(&msg.ty).await;
                let group = Arc::new(LeaderboardGroup::new(values));

                // Store the group and respond to the request
                service.groups.insert(msg.ty, group.clone());
                msg.tx.send(group).ok();
            })
        })
        .ok();
    }
}

pub struct LeaderboardLink(Link<Leaderboard>);

impl LeaderboardLink {
    pub fn start() -> LeaderboardLink {
        let leaderboard = Leaderboard::default();
        let link = leaderboard.start();
        LeaderboardLink(link)
    }

    pub async fn get(&self, ty: LeaderboardType) -> Option<Arc<LeaderboardGroup>> {
        let (tx, rx) = oneshot::channel();
        if self.0.do_send(GetRequest { ty, tx }).is_err() {
            return None;
        }
        rx.await.ok()
    }
}

impl Leaderboard {
    /// Computes the ranking values for the provided `ty` this consists of
    /// streaming the values from the database in chunks of 20, processing the
    /// chunks converting them into entries then sorting the entries based
    /// on their value.
    ///
    /// `ty` The leaderboard type
    async fn compute(&self, ty: &LeaderboardType) -> Vec<LeaderboardEntry> {
        // The amount of players to process in each database request
        const BATCH_COUNT: u64 = 20;

        let db = GlobalState::database();

        // The current database batch offset position
        let mut offset = 0;
        let mut values: Vec<LeaderboardEntry> = Vec::new();

        // Decide the ranking function to use based on the type
        let ranking_fn: Box<dyn Ranker> = ty.into();

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
    let (classes, characters) = try_join!(player.get_classes(&db), player.get_characters(&db),)?;

    let classes: Vec<_> = classes
        .iter()
        .filter_map(|value| parse_player_class(&value.value))
        .collect();

    let characters: Vec<_> = characters
        .iter()
        .filter_map(|value| parse_player_character(&value.value))
        .collect();

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
