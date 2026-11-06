use sea_orm::{DatabaseConnection, EntityTrait, IntoActiveModel, QuerySelect};

use crate::database::{
    DbResult,
    entities::{galaxy_at_war, game_report, leaderboard_data, player_data, players},
};

/// Performs a paginated copy of the provided model from the source database to the destination database
/// assumes that any required relations already exist in the database
async fn paginate_entity_copy<E: EntityTrait>(
    source: &DatabaseConnection,
    dest: &DatabaseConnection,
    page_size: usize,
) -> DbResult<usize>
where
    E::Model: IntoActiveModel<E::ActiveModel>,
{
    let limit = page_size as u64;
    let mut offset = 0;

    let mut copied = 0;

    loop {
        let entities = E::find().limit(limit).offset(offset).all(source).await?;
        let count = entities.len();

        let active_models = entities.into_iter().map(|value| value.into_active_model());
        E::insert_many(active_models).exec(dest).await?;
        copied += count;

        if count < page_size {
            break;
        }

        log::info!("processed {copied}");
        offset += limit;
    }

    Ok(copied)
}

/// Copies all the entities from the source database to the destination database
pub async fn copy_database(source: &DatabaseConnection, dest: &DatabaseConnection) -> DbResult<()> {
    let page_size = 1000;
    log::info!("copying players");
    let count = paginate_entity_copy::<players::Entity>(source, dest, page_size).await?;
    log::info!("copied players {count} entries(s)");

    log::info!("copying galaxy at war");
    let count = paginate_entity_copy::<galaxy_at_war::Entity>(source, dest, page_size).await?;
    log::info!("copied galaxy at war {count} entries(s)");

    log::info!("copying player data");
    let count = paginate_entity_copy::<player_data::Entity>(source, dest, page_size).await?;
    log::info!("copied player data {count} entries(s)");

    log::info!("copying leaderboard data");
    let count = paginate_entity_copy::<leaderboard_data::Entity>(source, dest, page_size).await?;
    log::info!("copied leaderboard {count} entries(s)");

    log::info!("copying game report data");
    let count = paginate_entity_copy::<game_report::Entity>(source, dest, page_size).await?;
    log::info!("copied game report {count} entries(s)");

    Ok(())
}
