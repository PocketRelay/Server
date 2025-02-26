//! SeaORM Entity. Generated by sea-orm-codegen 0.9.3

use crate::database::DbResult;
use crate::utils::types::PlayerID;
use sea_orm::sea_query::OnConflict;
use sea_orm::ActiveValue::NotSet;
use sea_orm::{prelude::*, FromQueryResult, InsertResult, QueryOrder, QuerySelect};
use sea_orm::{ActiveValue::Set, DatabaseConnection, EntityTrait};
use serde::{Deserialize, Serialize};
use std::future::Future;

#[derive(Serialize, Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "leaderboard_data")]
pub struct Model {
    /// Unique Identifier for the entry
    #[sea_orm(primary_key)]
    #[serde(skip)]
    pub id: u32,
    /// The type of leaderboard this data is for
    #[serde(skip)]
    pub ty: LeaderboardType,
    /// ID of the player this data is for
    pub player_id: PlayerID,
    /// The value of this leaderboard data
    pub value: u32,
}

/// Type of leaderboard entity
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "u8", db_type = "TinyUnsigned")]
#[repr(u8)]
pub enum LeaderboardType {
    /// Leaderboard based on the player N7 ratings
    #[serde(rename = "n7")]
    #[sea_orm(num_value = 0)]
    N7Rating = 0,
    /// Leaderboard based on the player challenge point number
    #[serde(rename = "cp")]
    #[sea_orm(num_value = 1)]
    ChallengePoints = 1,
}

#[derive(FromQueryResult, Serialize)]
pub struct LeaderboardDataAndRank {
    /// Unique Identifier for the entry
    #[serde(skip)]
    #[allow(unused)]
    pub id: u32,
    /// ID of the player this data is for
    pub player_id: PlayerID,
    /// The name of the player this entry is for
    pub player_name: String,
    /// The value of this leaderboard data
    pub value: u32,
    /// The ranking of this entry (Position in the leaderboard)
    pub rank: u32,
}

impl Model {
    /// Expression used to rank the leaderboard data
    const RANK_EXPR: &'static str = "RANK() OVER (ORDER BY value DESC) rank";
    /// The name of the column used for the rank value
    const RANK_COL: &'static str = "rank";
    /// The name of the column to store the loaded player name
    const PLAYER_NAME_COL: &'static str = "player_name";

    /// Counts the number of leaderboard data models for the
    /// specific `ty` type of leaderboard
    pub fn count(
        db: &DatabaseConnection,
        ty: LeaderboardType,
    ) -> impl Future<Output = DbResult<u64>> + Send + '_ {
        Entity::find()
            // Filter by the type
            .filter(Column::Ty.eq(ty))
            // Get the number of items
            .count(db)
    }

    /// Gets a collection of leaderboard data for the specific
    /// `ty` type of leaderboard starting with the `start` rank
    /// and including maximum of `count` entries
    pub fn get_offset(
        db: &DatabaseConnection,
        ty: LeaderboardType,
        start: u32,
        count: u32,
    ) -> impl Future<Output = DbResult<Vec<LeaderboardDataAndRank>>> + Send + '_ {
        Entity::find()
            // Add the ranking expression
            .expr(Expr::cust(Self::RANK_EXPR))
            // Filter by the type
            .filter(Column::Ty.eq(ty))
            // Order lowest to highest ranking
            .order_by_asc(Expr::cust(Self::RANK_COL))
            // Offset to the starting position
            .offset(start as u64)
            // Only take the requested amount
            .limit(count as u64)
            // Inner join on the players table
            .join(sea_orm::JoinType::InnerJoin, Relation::Player.def())
            // Use the player name from the players table
            .column_as(super::players::Column::DisplayName, Self::PLAYER_NAME_COL)
            // Turn it into the new model
            .into_model::<LeaderboardDataAndRank>()
            // Collect all the matching entities
            .all(db)
    }

    /// Gets the leaderboard data for a specific player on a
    /// specific leaderboard type
    pub fn get_entry(
        db: &DatabaseConnection,
        ty: LeaderboardType,
        player_id: PlayerID,
    ) -> impl Future<Output = DbResult<Option<LeaderboardDataAndRank>>> + Send + '_ {
        Entity::find()
            // Add the ranking expression
            .expr(Expr::cust(Self::RANK_EXPR))
            // Filter by the type and the specific player ID
            .filter(Column::Ty.eq(ty).and(Column::PlayerId.eq(player_id)))
            // Order lowest to highest ranking
            .order_by_asc(Expr::cust(Self::RANK_COL))
            // Inner join on the players table
            .join(sea_orm::JoinType::InnerJoin, Relation::Player.def())
            // Use the player name from the players table
            .column_as(super::players::Column::DisplayName, Self::PLAYER_NAME_COL)
            // Turn it into the new model
            .into_model::<LeaderboardDataAndRank>()
            // Collect all the matching entities
            .one(db)
    }

    /// Gets a collection of leaderboard data for the specific
    /// `ty` type of leaderboard including only the players
    /// in the provided `player_ids` collection
    pub fn get_filtered(
        db: &DatabaseConnection,
        ty: LeaderboardType,
        player_ids: Vec<PlayerID>,
    ) -> impl Future<Output = DbResult<Vec<LeaderboardDataAndRank>>> + Send + '_ {
        Entity::find()
            // Add the ranking expression
            .expr(Expr::cust(Self::RANK_EXPR))
            // Filter by the type and the requested player IDs
            .filter(Column::Ty.eq(ty).and(Column::PlayerId.is_in(player_ids)))
            // Order lowest to highest ranking
            .order_by_asc(Expr::cust(Self::RANK_COL))
            // Inner join on the players table
            .join(sea_orm::JoinType::InnerJoin, Relation::Player.def())
            // Use the player name from the players table
            .column_as(super::players::Column::DisplayName, Self::PLAYER_NAME_COL)
            // Turn it into the new model
            .into_model::<LeaderboardDataAndRank>()
            // Collect all the matching entities
            .all(db)
    }

    /// Gets a collection of leaderboard data for the specific
    /// `ty` type of leaderboard including maximum of `count` entries
    /// centering the results around the rank of the provided `player_id`
    pub async fn get_centered(
        db: &DatabaseConnection,
        ty: LeaderboardType,
        player_id: PlayerID,
        count: u32,
    ) -> DbResult<Option<Vec<LeaderboardDataAndRank>>> {
        // Find the entry we are centering on
        let value = match Self::get_entry(db, ty, player_id).await? {
            Some(value) => value,
            // The specified player hasn't been ranked
            None => return Ok(None),
        };

        // The number of ranks to start at before the centered rank
        let before = (count / 2)
            // Add 1 when the count is even
            .saturating_add((count % 2 == 0) as u32);

        // Determine the starting rank saturating zero bounds
        let start = value.rank.saturating_sub(before);

        let values = Self::get_offset(db, ty, start, count).await?;
        Ok(Some(values))
    }

    /// Function providing the conflict handling for upserting
    /// values into the leaderboard data
    #[inline(always)]
    fn conflict_handle() -> OnConflict {
        // Update the value column if the player ID in that type already exists
        OnConflict::columns([Column::PlayerId, Column::Ty])
            .update_column(Column::Value)
            .to_owned()
    }

    /// Sets the leaderboard value for the specified `player_id` on
    /// a specific leaderboard `ty` type to the provided `value`
    #[allow(unused)]
    pub fn set(
        db: &DatabaseConnection,
        ty: LeaderboardType,
        player_id: PlayerID,
        value: u32,
    ) -> impl Future<Output = DbResult<InsertResult<ActiveModel>>> + Send + '_ {
        Entity::insert(ActiveModel {
            id: NotSet,
            ty: Set(ty),
            player_id: Set(player_id),
            value: Set(value),
        })
        .on_conflict(Self::conflict_handle())
        .exec(db)
    }

    /// Bulk updates the values for each player ID -> value pair on
    /// the provided `ty` leaderboard
    pub fn set_ty_bulk(
        db: &DatabaseConnection,
        ty: LeaderboardType,
        data: impl Iterator<Item = (PlayerID, u32)>,
    ) -> impl Future<Output = DbResult<InsertResult<ActiveModel>>> + Send + '_ {
        // Insert all the models
        Entity::insert_many(
            // Transform the key value pairs into insertable models
            data.map(|(player_id, value)| ActiveModel {
                id: NotSet,
                ty: Set(ty),
                player_id: Set(player_id),
                value: Set(value),
            }),
        )
        .on_conflict(Self::conflict_handle())
        .exec(db)
    }
}

impl From<&str> for LeaderboardType {
    fn from(value: &str) -> Self {
        if value.starts_with("N7Rating") {
            Self::N7Rating
        } else {
            Self::ChallengePoints
        }
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::players::Entity",
        from = "Column::PlayerId",
        to = "super::players::Column::Id"
    )]
    Player,
}

// `Related` trait has to be implemented by hand
impl Related<super::players::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Player.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
