use chrono::Local;
use rand::{distributions::Uniform, Rng};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
};
use tokio::{task::JoinSet, try_join};

use crate::{
    database::entities::{
        galaxy_at_war::ActiveModel as GawActiveModel, leaderboard_data::LeaderboardType,
        players::ActiveModel as PlayerActiveModel, LeaderboardData, PlayerData, PlayerRole,
    },
    utils::hashing::hash_password,
};
use std::fmt::Write;

use super::connect_database;

/// The number of users to seed
const SEED_PLAYERS_COUNT: u32 = 10_000;

/// Class names to seed
static CLASS_NAMES: &[&str] = &[
    "Adept",
    "Soldier",
    "Engineer",
    "Sentinel",
    "Infiltrator",
    "Vanguard",
];

static CHARACTER_DATA: &[&str] = &[
    "20;4;AdeptHumanMale;MAdept;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;AdeptHumanFemale;FAdept;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;AdeptAsari;Asari;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;AdeptDrell;Drell;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;AdeptAsariCommando;Asari;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;AdeptHumanMaleCerberus;Human Male;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;AdeptN7;N7 Fury;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;AdeptVolus;Volus Adept;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;AdeptKrogan;Krogan Shaman;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;AdeptBatarian;Krogan Shaman;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;AdeptCollector;Awakened Collector;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SoldierHumanMale;MSoldier;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SoldierHumanFemale;FSoldier;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SoldierKrogan;Krogan;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SoldierTurian;Turian;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SoldierHumanMaleBF3;MSoldier;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SoldierBatarian;Batarian;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SoldierVorcha;Vorcha;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SoldierN7;N7 Destroyer;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;N7SoldierTurian;Turian Havoc;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SoldierGeth;Geth Trooper;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SoldierMQuarian;Geth Trooper;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SoldierGethDestroyer;Geth Juggernaut;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;EngineerHumanMale;MEngineer;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;EngineerHumanFemale;FEngineer;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;EngineerQuarian;FEngineer;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;EngineerSalarian;Salarian;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;EngineerGeth;Geth;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;EngineerQuarianMale;Quarian Male;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;EngineerN7;N7 Demolisher;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;EngineerVolus;Volus Engineer;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;EngineerTurian;Turian Saboteur;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;EngineerVorcha;Turian Saboteur;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;EngineerMerc;Talon Mercenary;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SentinelHumanMale;MSentinel;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SentinelHumanFemale;FSentinel;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SentinelTurian;Turian;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SentinelKrogan;Krogan;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SentinelBatarian;Batarian;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SentinelVorcha;Vorcha;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SentinelN7;N7 Paladin;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SentinelVolus;Volus Mercenary;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SentinelAsari;Volus Mercenary;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;SentinelKroganWarlord;Krogan Warlord;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;InfiltratorHumanMale;MInfiltrate;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;InfiltratorHumanFemale;FInfiltrate;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;InfiltratorSalarian;Salarian;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;InfiltratorQuarian;Quarian;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;InfiltratorGeth;Geth;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;InfiltratorQuarianMale;Quarian Male;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;InfiltratorN7;N7 Shadow;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;N7InfiltratorTurian;Turian Ghost;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;InfiltratorDrell;Drell Assassin;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;InfiltratorAsari;Drell Assassin;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;InfiltratorFembot;Krogan Warlord;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;InfiltratorHumanFemaleBF3;MSoldier;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;VanguardHumanMale;MVanguard;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;VanguardHumanFemale;FVanguard;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;VanguardDrell;Drell;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;VanguardAsari;Asari;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;VanguardKrogan;Krogan;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;VanguardHumanMaleCerberus;Human Male;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;VanguardN7;N7 Slayer;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;VanguardVolus;Volus Protector;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;VanguardBatarian;Volus Protector;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
    "20;4;VanguardTurianFemale;Cabal Vanguard;0;45;0;47;45;9;9;0;0;0;0;0;;;;;False;True",
];

/// Seeds the database with a collection of players and their associated
/// player data. Ensure the database is empty before seeding as to not
/// cause conflicts.
///
/// Models are seeded 1 by 1 as memory usage could greatly increase for
/// larger seeding batches
#[tokio::test]
#[ignore]
pub async fn seed() {
    let db = connect_database().await;

    // All accounts use the same default password
    let default_password = hash_password("test").unwrap();

    let current_time = Local::now().naive_local();

    let mut rng = rand::thread_rng();

    // Random sample used for role data
    let role_sample = Uniform::new_inclusive(0, 3);
    // Class level sample
    let level_sample = Uniform::new_inclusive(0, 20);
    // Random sample used for gaw groups
    let gaw_sample = Uniform::new_inclusive(5000, 10099);

    const INVENTORY_LENGTH: usize = 671;

    let mut join_set: JoinSet<()> = JoinSet::new();

    for i in 0..SEED_PLAYERS_COUNT {
        println!("Seeding player {i}");

        let email = format!("test{i}@test.com");
        let display_name = format!("Test {i}");
        let password = default_password.clone();

        // Randomly decide if the user should be an admin
        let role = match rng.sample(role_sample) {
            3 => PlayerRole::Admin,
            _ => PlayerRole::Default,
        };

        // Create the new player account
        let model = PlayerActiveModel {
            id: NotSet,
            email: Set(email),
            display_name: Set(display_name),
            password: Set(Some(password)),
            role: Set(role),
        }
        .insert(&db)
        .await
        .unwrap();

        // Set the player leaderboard data
        try_join!(
            LeaderboardData::set(&db, LeaderboardType::N7Rating, model.id, rng.gen()),
            LeaderboardData::set(&db, LeaderboardType::ChallengePoints, model.id, rng.gen())
        )
        .unwrap();

        // Create galaxy at war data for the player
        GawActiveModel {
            id: NotSet,
            last_modified: Set(current_time),
            player_id: Set(model.id),
            group_a: Set(rng.sample(gaw_sample)),
            group_b: Set(rng.sample(gaw_sample)),
            group_c: Set(rng.sample(gaw_sample)),
            group_d: Set(rng.sample(gaw_sample)),
            group_e: Set(rng.sample(gaw_sample)),
        }
        .insert(&db)
        .await
        .unwrap();

        let mut player_data: Vec<(String, String)> = Vec::new();

        {
            let mut inventory: String = String::with_capacity(INVENTORY_LENGTH * 2);
            for _ in 0..INVENTORY_LENGTH {
                // Generate a random value for the inventory item
                let value: u8 = rng.gen();

                // Store the value as a 2 char hex value
                write!(&mut inventory, "{value:2x}").unwrap();
            }

            let credits: u32 = rng.gen();
            let credits_spent: u32 = rng.gen();
            let games_played: u32 = rng.gen();
            let seconds_played: u32 = rng.gen();

            // Generate the player base data
            let base_data = format!(
                "20;4;{credits};-1;0;{credits_spent};0;{games_played};{seconds_played};0;{inventory}"
            );

            player_data.push(("Base".to_string(), base_data));
        }

        // Set the player class data for each class
        for (index, class_name) in CLASS_NAMES.iter().enumerate() {
            let level: u32 = rng.sample(level_sample);
            let xp: f32 = rng.gen();

            let key = format!("class{}", index + 1);
            let value = format!("20;4;{class_name};{level};{xp:.4};0");
            player_data.push((key, value));
        }

        // Seed Completion data
        {
            let mut completion = String::from("22");

            for _ in 0..746 {
                let value: u8 = rng.gen();
                write!(&mut completion, ",{value}").unwrap();
            }

            player_data.push(("Completion".to_string(), completion));
        }

        // Seed cscompletion data
        {
            let mut completion = String::from("22");

            for _ in 0..221 {
                let value: u8 = rng.gen();
                write!(&mut completion, ",{value}").unwrap();
            }

            player_data.push(("cscompletion".to_string(), completion));
        }

        // Seed cstimestamps data
        {
            let mut value = String::new();

            for _ in 0..250 {
                let rand: u32 = rng.gen();
                write!(&mut value, "{rand},").unwrap();
            }

            // Pop trailing comma
            value.pop();
            player_data.push(("cstimestamps".to_string(), value));
        }

        // Seed cstimestamps2 data
        {
            let mut value = String::new();

            for _ in 0..250 {
                let rand: u32 = rng.gen();
                write!(&mut value, "{rand},").unwrap();
            }

            // Pop trailing comma
            value.pop();
            player_data.push(("cstimestamps2".to_string(), value));
        }

        // Seed cstimestamps3 data
        {
            let mut value = String::new();

            for _ in 0..245 {
                let rand: u32 = rng.gen();
                write!(&mut value, "{rand},").unwrap();
            }

            // Pop trailing comma
            value.pop();
            player_data.push(("cstimestamps3".to_string(), value));
        }

        // Completion,"22,0,1,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,1,0,0,1,1,1,0,1,0,0,0,1,0,1,1,0,0,1,0,0,1,0,0,0,0,1,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0"

        // Seed Progress data
        {
            let mut progress = String::from("22");

            for _ in 0..745 {
                let value: u32 = rng.gen();
                write!(&mut progress, ",{value}").unwrap();
            }
            player_data.push(("Progress".to_string(), progress));
        }

        // TODO: Seed random banner from known range
        player_data.push(("csreward".to_string(), 0.to_string()));
        player_data.push(("FaceCodes".to_string(), "20;".to_string()));
        // TODO: Random seed new item
        player_data.push(("NewItem".to_string(), "20;4;12 271".to_string()));

        // Seed character data
        for (index, value) in CHARACTER_DATA.iter().enumerate() {
            let key = format!("char{index}");
            player_data.push((key, value.to_string()));
        }

        let db2 = db.clone();

        join_set.spawn(async move {
            PlayerData::set_bulk(&db2, model.id, player_data.into_iter())
                .await
                .unwrap();
        });
    }

    while let Some(_) = join_set.join_next().await {}
}
