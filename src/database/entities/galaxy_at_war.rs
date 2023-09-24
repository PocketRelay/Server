//! SeaORM Entity. Generated by sea-orm-codegen 0.9.3

use crate::{database::DbResult, utils::types::PlayerID};
use chrono::Local;
use chrono::NaiveDateTime;
use sea_orm::prelude::*;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    DatabaseConnection,
};
use serde::Serialize;
use std::future::Future;

/// Structure for a galaxy at war model stored in the database
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "galaxy_at_war")]
pub struct Model {
    /// The unique ID for this galaxy at war data
    #[sea_orm(primary_key)]
    #[serde(skip)]
    pub id: u32,
    /// The ID of the player this galaxy at war data belongs to
    #[serde(skip)]
    pub player_id: u32,
    /// The time at which this galaxy at war data was last modified. Used
    /// to calculate how many days of decay have passed
    pub last_modified: NaiveDateTime,
    /// The first group value
    pub group_a: u16,
    /// The second group value
    pub group_b: u16,
    /// The third group value
    pub group_c: u16,
    /// The fourth group value
    pub group_d: u16,
    /// The fifth group value
    pub group_e: u16,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// The minimum value for galaxy at war entries
    const MIN_VALUE: u16 = 5000;
    /// The maximum value for galaxy at war entries
    const MAX_VALUE: u16 = 10099;

    pub async fn get(db: &DatabaseConnection, player_id: PlayerID) -> DbResult<Self> {
        let existing = Entity::find()
            .filter(Column::PlayerId.eq(player_id))
            .one(db)
            .await?;

        if let Some(value) = existing {
            return Ok(value);
        }

        let current_time = Local::now().naive_local();
        ActiveModel {
            id: NotSet,
            player_id: Set(player_id),
            last_modified: Set(current_time),
            group_a: Set(Self::MIN_VALUE),
            group_b: Set(Self::MIN_VALUE),
            group_c: Set(Self::MIN_VALUE),
            group_d: Set(Self::MIN_VALUE),
            group_e: Set(Self::MIN_VALUE),
        }
        .insert(db)
        .await
    }

    /// Increases the stored group values increasing them by the `values`
    /// provided for each respective group
    pub fn add(
        self,
        db: &DatabaseConnection,
        values: [u16; 5],
    ) -> impl Future<Output = DbResult<Self>> + '_ {
        self.transform(db, |a, b| a.saturating_add(b).min(Model::MAX_VALUE), values)
    }

    /// Decrease the stored group values decreasuing them by the `values`
    /// provided for each respective group
    pub fn sub(
        self,
        db: &DatabaseConnection,
        values: [u16; 5],
    ) -> impl Future<Output = DbResult<Self>> + '_ {
        self.transform(db, |a, b| a.saturating_sub(b).max(Model::MIN_VALUE), values)
    }

    /// Transforms the underlying group values using the provided action
    /// function which is given the current value as the first argument
    /// and the respective value from `values` as the second argument
    #[inline]
    pub async fn transform<F>(
        self,
        db: &DatabaseConnection,
        action: F,
        values: [u16; 5],
    ) -> DbResult<Self>
    where
        F: Fn(u16, u16) -> u16,
    {
        let current_time = Local::now().naive_local();
        ActiveModel {
            id: Set(self.id),
            player_id: Set(self.player_id),
            last_modified: Set(current_time),
            group_a: Set(action(self.group_a, values[0])),
            group_b: Set(action(self.group_b, values[1])),
            group_c: Set(action(self.group_c, values[2])),
            group_d: Set(action(self.group_d, values[3])),
            group_e: Set(action(self.group_e, values[4])),
        }
        .update(db)
        .await
    }

    /// Applies the daily decay progress to the group values calculating the
    /// decay amount from the number of days passed
    pub async fn apply_decay(self, db: &DatabaseConnection, decay: f32) -> DbResult<Self> {
        // Skip decaying if decay is non existent
        if decay <= 0.0 {
            return Ok(self);
        }

        let current_time = Local::now().naive_local();
        let days_passed = (current_time - self.last_modified).num_days() as f32;
        let decay_value = (decay * days_passed * 100.0) as u16;

        self.sub(db, [decay_value; 5]).await
    }
}
