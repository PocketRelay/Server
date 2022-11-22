use crate::{
    entities::{galaxy_at_war, player_classes, players},
    DbResult,
};
use chrono::Local;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    DatabaseConnection, IntoActiveModel, ModelTrait,
};
use std::cmp;

impl galaxy_at_war::Model {
    /// The minimum value for galaxy at war entries
    const MIN_VALUE: u16 = 5000;
    /// The maximum value for galaxy at war entries
    const MAX_VALUE: u16 = 10099;

    /// Finds the total number of promotions that the
    /// provided player has returning zero on failure
    ///
    /// `db`     The database instance
    /// `player` The player to get promotions for
    pub async fn find_promotions(db: &DatabaseConnection, player: &players::Model) -> u32 {
        let Ok(classes) = player
            .find_related(player_classes::Entity)
            .all(db)
            .await else {

            return 0;
        };
        let promotions = classes.iter().map(|value| value.promotions).sum();
        promotions
    }

    /// Finds or creates a new galaxy at war entry for the provided
    /// player. If one exists then the provided decay value will be
    /// applied to it.
    ///
    /// `db`     The database connection
    /// `player` The player to search for galaxy at war models for
    /// `decay`  The decay value
    pub async fn find_or_create(
        db: &DatabaseConnection,
        player: &players::Model,
        decay: f32,
    ) -> DbResult<Self> {
        let existing = player.find_related(galaxy_at_war::Entity).one(db).await?;
        if let Some(value) = existing {
            return value.apply_decay(db, decay).await;
        }

        let current_time = Local::now().naive_local();
        let model = galaxy_at_war::ActiveModel {
            id: NotSet,
            player_id: Set(player.id),
            last_modified: Set(current_time),
            group_a: Set(Self::MIN_VALUE),
            group_b: Set(Self::MIN_VALUE),
            group_c: Set(Self::MIN_VALUE),
            group_d: Set(Self::MIN_VALUE),
            group_e: Set(Self::MIN_VALUE),
        };

        model.insert(db).await
    }

    /// Increases the group values stored on the provided
    /// galaxy at war models by the values provided.
    ///
    /// `db`     The database connection
    /// `value`  The galaxy at war model to increase
    /// `values` The values to increase each group by
    pub async fn increase(
        self,
        db: &DatabaseConnection,
        values: (u16, u16, u16, u16, u16),
    ) -> DbResult<galaxy_at_war::Model> {
        let mut gaw_data = self.into_active_model();
        gaw_data.group_a = Set(cmp::min(values.0, Self::MAX_VALUE));
        gaw_data.group_b = Set(cmp::min(values.1, Self::MAX_VALUE));
        gaw_data.group_c = Set(cmp::min(values.2, Self::MAX_VALUE));
        gaw_data.group_d = Set(cmp::min(values.3, Self::MAX_VALUE));
        gaw_data.group_e = Set(cmp::min(values.4, Self::MAX_VALUE));
        gaw_data.update(db).await
    }

    /// Applies the provided galaxy at war decay value to the provided
    /// galaxy at war model decreasing the values by the number of days
    /// that have passed.
    ///
    /// `db`    The database connection
    /// `value` The galaxy at war model to decay
    /// `decay` The decay value
    async fn apply_decay(self, db: &DatabaseConnection, decay: f32) -> DbResult<Self> {
        // Skip decaying if decay is non existent
        if decay <= 0.0 {
            return Ok(self);
        }

        let current_time = Local::now().naive_local();
        let days_passed = (current_time - self.last_modified).num_days() as f32;
        let decay_value = (decay * days_passed * 100.0) as u16;

        // Apply decay while keeping minimum
        let a = cmp::max(self.group_a - decay_value, Self::MIN_VALUE);
        let b = cmp::max(self.group_b - decay_value, Self::MIN_VALUE);
        let c = cmp::max(self.group_c - decay_value, Self::MIN_VALUE);
        let d = cmp::max(self.group_d - decay_value, Self::MIN_VALUE);
        let e = cmp::max(self.group_e - decay_value, Self::MIN_VALUE);

        // Update stored copy
        let mut value = self.into_active_model();
        value.group_a = Set(a);
        value.group_b = Set(b);
        value.group_c = Set(c);
        value.group_d = Set(d);
        value.group_e = Set(e);

        value.update(db).await
    }
}
