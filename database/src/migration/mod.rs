pub use sea_orm_migration::prelude::*;

mod m20221015_142649_players_table;
mod m20221015_145431_players_characters_table;
mod m20221015_145458_players_classes_table;
mod m20221015_153750_galaxy_at_war_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20221015_142649_players_table::Migration),
            Box::new(m20221015_145431_players_characters_table::Migration),
            Box::new(m20221015_145458_players_classes_table::Migration),
            Box::new(m20221015_153750_galaxy_at_war_table::Migration),
        ]
    }
}
