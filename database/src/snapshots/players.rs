use sea_orm::DatabaseConnection;
use serde::Serialize;
use utils::types::PlayerID;

use crate::{DbResult, GalaxyAtWar, Player, PlayerCharacter, PlayerClass};

#[derive(Serialize)]
pub struct PlayerBasicSnapshot {
    pub id: PlayerID,
    pub email: String,
    pub display_name: String,
    pub origin: bool,
    pub credits: u32,
    pub credits_spent: u32,
    pub games_played: u32,
    pub seconds_played: u32,
    pub inventory: String,
    pub csreward: u16,
}

impl PlayerBasicSnapshot {
    pub fn take_snapshot(player: Player) -> Self {
        Self {
            id: player.id,
            email: player.email,
            display_name: player.display_name,
            origin: player.origin,
            credits: player.credits,
            credits_spent: player.credits_spent,
            games_played: player.games_played,
            seconds_played: player.seconds_played,
            inventory: player.inventory,
            csreward: player.csreward,
        }
    }
}

#[derive(Serialize)]
pub struct PlayerDeepSnapshot {
    pub id: PlayerID,
    pub email: String,
    pub display_name: String,
    pub origin: bool,
    pub credits: u32,
    pub credits_spent: u32,
    pub games_played: u32,
    pub seconds_played: u32,
    pub inventory: String,
    pub csreward: u16,
    pub face_codes: Option<String>,
    pub new_item: Option<String>,
    pub completion: Option<String>,
    pub progress: Option<String>,
    pub cs_completion: Option<String>,
    pub cs_timestamps1: Option<String>,
    pub cs_timestamps2: Option<String>,
    pub cs_timestamps3: Option<String>,
    pub classes: Vec<PlayerClass>,
    pub characters: Vec<PlayerCharacter>,
    pub galaxy_at_war: GalaxyAtWar,
}

impl PlayerDeepSnapshot {
    pub async fn take_snapshot(db: &DatabaseConnection, player: Player) -> DbResult<Self> {
        let classes = PlayerClass::find_all(db, &player).await?;
        let characters = PlayerCharacter::find_all(db, &player).await?;
        let galaxy_at_war = GalaxyAtWar::find_or_create(db, &player, 0.0).await?;
        Ok(Self {
            id: player.id,
            email: player.email,
            display_name: player.display_name,
            origin: player.origin,
            credits: player.credits,
            credits_spent: player.credits_spent,
            games_played: player.games_played,
            seconds_played: player.seconds_played,
            inventory: player.inventory,
            csreward: player.csreward,
            face_codes: player.face_codes,
            new_item: player.new_item,
            completion: player.completion,
            progress: player.progress,
            cs_completion: player.cs_completion,
            cs_timestamps1: player.cs_timestamps1,
            cs_timestamps2: player.cs_timestamps2,
            cs_timestamps3: player.cs_timestamps3,
            classes,
            characters,
            galaxy_at_war,
        })
    }
}
