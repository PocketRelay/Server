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

#[derive(FromQueryResult, Serialize)]
pub struct LeaderboardDataAndRank {
    /// Unique Identifier for the entry
    #[serde(skip)]
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
    pub fn total(
        db: &DatabaseConnection,
        ty: LeaderboardType,
    ) -> impl Future<Output = DbResult<u64>> + Send + '_ {
        Entity::find().filter(Column::Ty.eq(ty)).count(db)
    }

    pub fn get_offset(
        db: &DatabaseConnection,
        ty: LeaderboardType,
        start: u32,
        count: u32,
    ) -> impl Future<Output = DbResult<Vec<LeaderboardDataAndRank>>> + Send + '_ {
        Entity::find()
            // Ranking by the values
            .expr(Expr::cust("RANK () OVER (ORDER BY value DESC) rank"))
            // Filter by the type
            .filter(Column::Ty.eq(ty))
            // Order highest to lowest
            .order_by_desc(Expr::cust("rank"))
            // Offset to the starting position
            .offset(start as u64)
            // Only take the requested amouont
            .limit(count as u64)
            // Join the playe rname
            // Inner join on the player and use the player name
            .join(sea_orm::JoinType::InnerJoin, Relation::Player.def())
            .column_as(super::players::Column::DisplayName, "player_name")
            // Turn it into the new model
            .into_model::<LeaderboardDataAndRank>()
            // Collect all the matching entities
            .all(db)
    }

    pub async fn get_centered(
        db: &DatabaseConnection,
        ty: LeaderboardType,
        player_id: PlayerID,
        count: u32,
    ) -> DbResult<Option<Vec<LeaderboardDataAndRank>>> {
        let value = match Self::get_entry(db, ty, player_id).await? {
            Some(value) => value,
            None => return Ok(None),
        };

        if count == 0 {
            return Ok(None);
        }

        // The number of items before the center index
        let before = if count % 2 == 0 {
            (count / 2).saturating_add(1)
        } else {
            count / 2
        };

        let start = value.rank.saturating_sub(before);
        let values = Self::get_offset(db, ty, start, count).await?;
        Ok(Some(values))
    }

    pub fn get_entry(
        db: &DatabaseConnection,
        ty: LeaderboardType,
        player_id: PlayerID,
    ) -> impl Future<Output = DbResult<Option<LeaderboardDataAndRank>>> + Send + '_ {
        Entity::find()
            // Ranking by the values
            .expr(Expr::cust("RANK () OVER (ORDER BY value DESC) rank"))
            // Filter by the type
            .filter(Column::Ty.eq(ty).and(Column::PlayerId.eq(player_id)))
            // Order highest to lowest
            .order_by_desc(Expr::cust("rank"))
            // Join the playe rname
            // Inner join on the player and use the player name
            .join(sea_orm::JoinType::InnerJoin, Relation::Player.def())
            .column_as(super::players::Column::DisplayName, "player_name")
            // Turn it into the new model
            .into_model::<LeaderboardDataAndRank>()
            // Collect all the matching entities
            .one(db)
    }

    pub fn get_filtered(
        db: &DatabaseConnection,
        ty: LeaderboardType,
        player_ids: Vec<PlayerID>,
    ) -> impl Future<Output = DbResult<Vec<LeaderboardDataAndRank>>> + Send + '_ {
        Entity::find()
            // Ranking by the values
            .expr(Expr::cust("RANK () OVER (ORDER BY value DESC) rank"))
            // Filter by the type
            .filter(Column::Ty.eq(ty).and(Column::PlayerId.is_in(player_ids)))
            // Order highest to lowest
            .order_by_desc(Expr::cust("rank"))
            // Join the playe rname
            // Inner join on the player and use the player name
            .join(sea_orm::JoinType::InnerJoin, Relation::Player.def())
            .column_as(super::players::Column::DisplayName, "player_name")
            // Turn it into the new model
            .into_model::<LeaderboardDataAndRank>()
            // Collect all the matching entities
            .all(db)
    }

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
        .on_conflict(
            // Update the value column if a key already exists
            OnConflict::columns([Column::PlayerId, Column::Ty])
                .update_column(Column::Value)
                .to_owned(),
        )
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
        .on_conflict(
            // Update the value column if a key already exists
            OnConflict::columns([Column::PlayerId, Column::Ty])
                .update_column(Column::Value)
                .to_owned(),
        )
        .exec(db)
    }
}
