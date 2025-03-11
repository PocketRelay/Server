use std::str::Split;

use super::parser::{next_bool, next_float, next_int, next_str, next_string, ParseResult};

#[derive(Debug, PartialEq, Eq)]
pub struct PlayerCharacter {
    pub kit_name: String,
    pub character_name: String,
    pub tint_1_id: u32,
    pub tint_2_id: u32,
    pub pattern_id: u32,
    pub pattern_color_id: u32,
    pub phong_id: u32,
    pub emissive_id: u32,
    pub skin_tone_id: u32,
    pub seconds_played: u32,
    pub timestamp: PlayerCharacterTimestamp,
    pub powers: Vec<PlayerCharacterPower>,
    pub hotkeys: String,
    pub weapons: Vec<WeaponId>,
    pub weapon_mods: Vec<PlayerCharacterWeaponMod>,
    pub deployed: bool,
    pub leveled_up: bool,
}

pub type WeaponId = u32;
pub type WeaponModId = u32;

#[derive(Debug, PartialEq, Eq)]
pub struct PlayerCharacterTimestamp {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub seconds: u32,
}

#[derive(Debug, PartialEq)]
pub struct PlayerCharacterPower {
    pub power_name: String,
    pub power_id: u32,
    pub power_progress: f32,
    pub power_selections: [PowerSelectionPair; 3],
    pub wheel_display_index: u32,
    pub uses_talent_points: bool,
}

impl Eq for PlayerCharacterPower {}

#[derive(Debug, PartialEq, Eq)]
pub struct PowerSelectionPair {
    pub a: bool,
    pub b: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PlayerCharacterWeaponMod {
    pub weapon_id: u32,
    pub weapon_mod_ids: Vec<u32>,
}

impl PlayerCharacter {
    pub fn parse_input(input: &str) -> ParseResult<(u32, u32, PlayerCharacter)> {
        let mut p = input.split(';');
        let version = next_int(&mut p)?;
        let dev_version = next_int(&mut p)?;
        let character = PlayerCharacter::parse(&mut p)?;
        Ok((version, dev_version, character))
    }

    pub fn parse(p: &mut Split<'_, char>) -> ParseResult<PlayerCharacter> {
        let kit_name = next_string(p)?;
        let character_name = next_string(p)?;
        let tint_1_id = next_int(p)?;
        let tint_2_id = next_int(p)?;
        let pattern_id = next_int(p)?;
        let pattern_color_id = next_int(p)?;
        let phong_id = next_int(p)?;
        let emissive_id = next_int(p)?;
        let skin_tone_id = next_int(p)?;
        let seconds_played = next_int(p)?;
        let timestamp = PlayerCharacterTimestamp::parse(p)?;
        let powers = {
            let powers = next_str(p)?;
            PlayerCharacterPower::parse_list(powers)?
        };
        let hotkeys = next_string(p)?;
        let weapons = {
            let weapons = next_str(p)?;
            parse_weapons(weapons)?
        };
        let weapon_mods = {
            let weapon_mods = next_str(p)?;
            PlayerCharacterWeaponMod::parse_list(weapon_mods)?
        };
        let deployed = next_bool(p)?;
        let leveled_up = next_bool(p)?;

        Ok(PlayerCharacter {
            kit_name,
            character_name,
            tint_1_id,
            tint_2_id,
            pattern_id,
            pattern_color_id,
            phong_id,
            emissive_id,
            skin_tone_id,
            seconds_played,
            timestamp,
            powers,
            hotkeys,
            weapons,
            weapon_mods,
            deployed,
            leveled_up,
        })
    }
}

fn parse_weapons(input: &str) -> ParseResult<Vec<WeaponId>> {
    let mut weapons: Vec<u32> = Vec::new();
    let parts = input.split(',');
    for part in parts {
        let weapon_index: u32 = part.parse()?;
        weapons.push(weapon_index);
    }
    Ok(weapons)
}

impl PlayerCharacterTimestamp {
    pub fn parse(p: &mut Split<'_, char>) -> ParseResult<PlayerCharacterTimestamp> {
        let year = next_int(p)?;
        let month = next_int(p)?;
        let day = next_int(p)?;
        let seconds = next_int(p)?;

        Ok(PlayerCharacterTimestamp {
            year,
            month,
            day,
            seconds,
        })
    }
}

impl PlayerCharacterPower {
    /// Decodes an encoded player character power list from the provided input
    ///
    /// ```example
    /// Singularity 179 1.0000 0 0 0 0 0 0 0 True,Warp 185 0.0000 0 0 0 0 0 0 0 True,Shockwave 177 0.0000 0 0 0 0 0 0 0 True,MPPassive 206 0.0000 0 0 0 0 0 0 0 True,MPMeleePassive 200 0.0000 0 0 0 0 0 0 0 True,Consumable_Rocket 88 0.0000 0 0 0 0 0 0 0 False,Consumable_Revive 87 0.0000 0 0 0 0 0 0 0 False,Consumable_Shield 89 0.0000 0 0 0 0 0 0 0 False,Consumable_Ammo 86 0.0000 0 0 0 0 0 0 0 False
    /// ```
    pub fn parse_list(input: &str) -> ParseResult<Vec<PlayerCharacterPower>> {
        let mut powers = Vec::new();
        let p = input.split(',');

        for part in p {
            let power = PlayerCharacterPower::parse(part)?;
            powers.push(power);
        }

        Ok(powers)
    }

    /// Decodes an encoded player character power from the provided input
    ///
    /// ```example
    /// Singularity 179 1.0000 0 0 0 0 0 0 0 True
    /// ```
    pub fn parse(input: &str) -> ParseResult<PlayerCharacterPower> {
        let mut p = input.split(' ');
        let power_name = next_string(&mut p)?;
        let power_id: u32 = next_int(&mut p)?;
        let power_progress: f32 = next_float(&mut p)?;
        let power_selections = [
            PowerSelectionPair::parse(0, &mut p)?,
            PowerSelectionPair::parse(1, &mut p)?,
            PowerSelectionPair::parse(2, &mut p)?,
        ];
        let wheel_display_index = next_int(&mut p)?;
        let uses_talent_points = next_bool(&mut p)?;

        Ok(PlayerCharacterPower {
            power_name,
            power_id,
            power_progress,
            power_selections,
            wheel_display_index,
            uses_talent_points,
        })
    }
}

impl PowerSelectionPair {
    pub fn parse(index: u32, p: &mut Split<'_, char>) -> ParseResult<PowerSelectionPair> {
        let a: u32 = next_int(p)?;
        let b: u32 = next_int(p)?;

        let active_value: u32 = index + 1;

        let a = a == active_value;
        let b = b == active_value;

        Ok(PowerSelectionPair { a, b })
    }
}

impl PlayerCharacterWeaponMod {
    ///
    /// ```example
    /// 136 36 37,7 33
    /// ```
    pub fn parse_list(input: &str) -> ParseResult<Vec<PlayerCharacterWeaponMod>> {
        let mut powers = Vec::new();
        let p = input.split(',');

        for part in p {
            let power = PlayerCharacterWeaponMod::parse(part)?;
            powers.push(power);
        }

        Ok(powers)
    }

    ///
    /// ```example
    /// 136 36 37
    /// ```
    pub fn parse(input: &str) -> ParseResult<PlayerCharacterWeaponMod> {
        let mut p = input.split(' ');
        let weapon_id: u32 = next_int(&mut p)?;

        let mut weapon_mod_ids = Vec::new();
        for part in p {
            let weapon_mod_id = part.parse()?;
            weapon_mod_ids.push(weapon_mod_id)
        }

        Ok(PlayerCharacterWeaponMod {
            weapon_id,
            weapon_mod_ids,
        })
    }
}

#[cfg(test)]
mod test {

    use player_character::PlayerCharacterTimestamp;

    use crate::utils::parsing::player_character::{self, parse_weapons, PowerSelectionPair};

    use super::{PlayerCharacter, PlayerCharacterPower, PlayerCharacterWeaponMod};

    #[test]
    fn test_player_character_power() {
        let test_data = &[
            (
                "Singularity 179 6.0000 1 0 2 0 3 0 0 False",
                PlayerCharacterPower {
                    power_name: "Singularity".to_string(),
                    power_id: 179,
                    power_progress: 6.0000,
                    power_selections: [
                        PowerSelectionPair { a: true, b: false },
                        PowerSelectionPair { a: true, b: false },
                        PowerSelectionPair { a: true, b: false },
                    ],
                    wheel_display_index: 0,
                    uses_talent_points: false,
                },
            ),
            (
                "Warp 185 6.0000 0 1 0 2 0 3 0 False",
                PlayerCharacterPower {
                    power_name: "Warp".to_string(),
                    power_id: 185,
                    power_progress: 6.0000,
                    power_selections: [
                        PowerSelectionPair { a: false, b: true },
                        PowerSelectionPair { a: false, b: true },
                        PowerSelectionPair { a: false, b: true },
                    ],
                    wheel_display_index: 0,
                    uses_talent_points: false,
                },
            ),
            (
                "Shockwave 177 4.0000 1 0 0 0 0 0 0 False",
                PlayerCharacterPower {
                    power_name: "Shockwave".to_string(),
                    power_id: 177,
                    power_progress: 4.0000,
                    power_selections: [
                        PowerSelectionPair { a: true, b: false },
                        PowerSelectionPair { a: false, b: false },
                        PowerSelectionPair { a: false, b: false },
                    ],
                    wheel_display_index: 0,
                    uses_talent_points: false,
                },
            ),
            (
                "MPPassive 206 2.0000 0 0 0 0 0 0 0 False",
                PlayerCharacterPower {
                    power_name: "MPPassive".to_string(),
                    power_id: 206,
                    power_progress: 2.0000,
                    power_selections: [
                        PowerSelectionPair { a: false, b: false },
                        PowerSelectionPair { a: false, b: false },
                        PowerSelectionPair { a: false, b: false },
                    ],
                    wheel_display_index: 0,
                    uses_talent_points: false,
                },
            ),
            (
                "MPMeleePassive 200 3.0000 0 0 0 0 0 0 0 False",
                PlayerCharacterPower {
                    power_name: "MPMeleePassive".to_string(),
                    power_id: 200,
                    power_progress: 3.0000,
                    power_selections: [
                        PowerSelectionPair { a: false, b: false },
                        PowerSelectionPair { a: false, b: false },
                        PowerSelectionPair { a: false, b: false },
                    ],
                    wheel_display_index: 0,
                    uses_talent_points: false,
                },
            ),
            (
                "Consumable_Rocket 88 1.0000 0 0 0 0 0 0 0 False",
                PlayerCharacterPower {
                    power_name: "Consumable_Rocket".to_string(),
                    power_id: 88,
                    power_progress: 1.0000,
                    power_selections: [
                        PowerSelectionPair { a: false, b: false },
                        PowerSelectionPair { a: false, b: false },
                        PowerSelectionPair { a: false, b: false },
                    ],
                    wheel_display_index: 0,
                    uses_talent_points: false,
                },
            ),
            (
                "Consumable_Revive 87 1.0000 0 0 0 0 0 0 0 False",
                PlayerCharacterPower {
                    power_name: "Consumable_Revive".to_string(),
                    power_id: 87,
                    power_progress: 1.0000,
                    power_selections: [
                        PowerSelectionPair { a: false, b: false },
                        PowerSelectionPair { a: false, b: false },
                        PowerSelectionPair { a: false, b: false },
                    ],
                    wheel_display_index: 0,
                    uses_talent_points: false,
                },
            ),
            (
                "Consumable_Shield 89 1.0000 0 0 0 0 0 0 0 False",
                PlayerCharacterPower {
                    power_name: "Consumable_Shield".to_string(),
                    power_id: 89,
                    power_progress: 1.0000,
                    power_selections: [
                        PowerSelectionPair { a: false, b: false },
                        PowerSelectionPair { a: false, b: false },
                        PowerSelectionPair { a: false, b: false },
                    ],
                    wheel_display_index: 0,
                    uses_talent_points: false,
                },
            ),
            (
                "Consumable_Ammo 86 1.0000 0 0 0 0 0 0 0 False",
                PlayerCharacterPower {
                    power_name: "Consumable_Ammo".to_string(),
                    power_id: 86,
                    power_progress: 1.0000,
                    power_selections: [
                        PowerSelectionPair { a: false, b: false },
                        PowerSelectionPair { a: false, b: false },
                        PowerSelectionPair { a: false, b: false },
                    ],
                    wheel_display_index: 0,
                    uses_talent_points: false,
                },
            ),
        ];

        for (input, expected) in test_data {
            let power = PlayerCharacterPower::parse(input).unwrap();
            assert_eq!(power.power_name, expected.power_name);
            assert_eq!(power.power_id, expected.power_id);
            assert_eq!(power.power_progress, expected.power_progress);
            assert_eq!(power.power_selections, expected.power_selections);
            assert_eq!(power.wheel_display_index, expected.wheel_display_index);
            assert_eq!(power.uses_talent_points, expected.uses_talent_points);
        }
    }

    #[test]
    fn test_player_character_weapons() {
        let test_data: &[(&str, Vec<u32>)] = &[
            ("7,136", vec![7, 136]),
            ("12,136", vec![12, 136]),
            ("5,136", vec![5, 136]),
            ("7,15", vec![7, 15]),
            ("7", vec![7]),
            ("15", vec![15]),
        ];

        for (input, expected) in test_data {
            let weapons = parse_weapons(input).unwrap();
            assert_eq!(&weapons, expected);
        }
    }

    #[test]
    fn test_player_character_weapon_mods() {
        let test_data: &[(&str, PlayerCharacterWeaponMod)] = &[
            (
                "136 36 37",
                PlayerCharacterWeaponMod {
                    weapon_id: 136,
                    weapon_mod_ids: vec![36, 37],
                },
            ),
            (
                "7 33",
                PlayerCharacterWeaponMod {
                    weapon_id: 7,
                    weapon_mod_ids: vec![33],
                },
            ),
        ];

        for (input, expected) in test_data {
            let weapons_mod = PlayerCharacterWeaponMod::parse(input).unwrap();
            assert_eq!(&weapons_mod, expected);
        }
    }
    #[test]
    fn test_player_character_weapon_mods_list() {
        let test_data: &[(&str, Vec<PlayerCharacterWeaponMod>)] = &[(
            "136 36 37,7 33",
            vec![
                PlayerCharacterWeaponMod {
                    weapon_id: 136,
                    weapon_mod_ids: vec![36, 37],
                },
                PlayerCharacterWeaponMod {
                    weapon_id: 7,
                    weapon_mod_ids: vec![33],
                },
            ],
        )];

        for (input, expected) in test_data {
            let weapons_mods = PlayerCharacterWeaponMod::parse_list(input).unwrap();
            assert_eq!(&weapons_mods, expected);
        }
    }

    #[test]
    fn test_player_character() {
        let test_data = &[(
            "20;4;AdeptHumanMale;Test;0;45;0;47;45;9;9;0;0;0;0;0;\
            Singularity 179 6.0000 1 0 2 0 3 0 0 False,Warp 185 \
            6.0000 0 1 0 2 0 3 0 False,Shockwave 177 4.0000 1 0 0 \
            0 0 0 0 False,MPPassive 206 2.0000 0 0 0 0 0 0 0 False,\
            MPMeleePassive 200 3.0000 0 0 0 0 0 0 0 False,Consumable_Rocket \
            88 1.0000 0 0 0 0 0 0 0 False,Consumable_Revive 87 1.0000 0 0 0 0 0 0 0 False,\
            Consumable_Shield 89 1.0000 0 0 0 0 0 0 0 False,Consumable_Ammo \
            86 1.0000 0 0 0 0 0 0 0 False;;7,136;136 36 37,7 33;True;True",
            (
                20,
                4,
                PlayerCharacter {
                    kit_name: "AdeptHumanMale".to_string(),
                    character_name: "Test".to_string(),
                    tint_1_id: 0,
                    tint_2_id: 45,
                    pattern_id: 0,
                    pattern_color_id: 47,
                    phong_id: 45,
                    emissive_id: 9,
                    skin_tone_id: 9,
                    seconds_played: 0,
                    timestamp: PlayerCharacterTimestamp {
                        year: 0,
                        month: 0,
                        day: 0,
                        seconds: 0,
                    },
                    powers: vec![
                        PlayerCharacterPower {
                            power_name: "Singularity".to_string(),
                            power_id: 179,
                            power_progress: 6.0000,
                            power_selections: [
                                PowerSelectionPair { a: true, b: false },
                                PowerSelectionPair { a: true, b: false },
                                PowerSelectionPair { a: true, b: false },
                            ],
                            wheel_display_index: 0,
                            uses_talent_points: false,
                        },
                        PlayerCharacterPower {
                            power_name: "Warp".to_string(),
                            power_id: 185,
                            power_progress: 6.0000,
                            power_selections: [
                                PowerSelectionPair { a: false, b: true },
                                PowerSelectionPair { a: false, b: true },
                                PowerSelectionPair { a: false, b: true },
                            ],
                            wheel_display_index: 0,
                            uses_talent_points: false,
                        },
                        PlayerCharacterPower {
                            power_name: "Shockwave".to_string(),
                            power_id: 177,
                            power_progress: 4.0000,
                            power_selections: [
                                PowerSelectionPair { a: true, b: false },
                                PowerSelectionPair { a: false, b: false },
                                PowerSelectionPair { a: false, b: false },
                            ],
                            wheel_display_index: 0,
                            uses_talent_points: false,
                        },
                        PlayerCharacterPower {
                            power_name: "MPPassive".to_string(),
                            power_id: 206,
                            power_progress: 2.0000,
                            power_selections: [
                                PowerSelectionPair { a: false, b: false },
                                PowerSelectionPair { a: false, b: false },
                                PowerSelectionPair { a: false, b: false },
                            ],
                            wheel_display_index: 0,
                            uses_talent_points: false,
                        },
                        PlayerCharacterPower {
                            power_name: "MPMeleePassive".to_string(),
                            power_id: 200,
                            power_progress: 3.0000,
                            power_selections: [
                                PowerSelectionPair { a: false, b: false },
                                PowerSelectionPair { a: false, b: false },
                                PowerSelectionPair { a: false, b: false },
                            ],
                            wheel_display_index: 0,
                            uses_talent_points: false,
                        },
                        PlayerCharacterPower {
                            power_name: "Consumable_Rocket".to_string(),
                            power_id: 88,
                            power_progress: 1.0000,
                            power_selections: [
                                PowerSelectionPair { a: false, b: false },
                                PowerSelectionPair { a: false, b: false },
                                PowerSelectionPair { a: false, b: false },
                            ],
                            wheel_display_index: 0,
                            uses_talent_points: false,
                        },
                        PlayerCharacterPower {
                            power_name: "Consumable_Revive".to_string(),
                            power_id: 87,
                            power_progress: 1.0000,
                            power_selections: [
                                PowerSelectionPair { a: false, b: false },
                                PowerSelectionPair { a: false, b: false },
                                PowerSelectionPair { a: false, b: false },
                            ],
                            wheel_display_index: 0,
                            uses_talent_points: false,
                        },
                        PlayerCharacterPower {
                            power_name: "Consumable_Shield".to_string(),
                            power_id: 89,
                            power_progress: 1.0000,
                            power_selections: [
                                PowerSelectionPair { a: false, b: false },
                                PowerSelectionPair { a: false, b: false },
                                PowerSelectionPair { a: false, b: false },
                            ],
                            wheel_display_index: 0,
                            uses_talent_points: false,
                        },
                        PlayerCharacterPower {
                            power_name: "Consumable_Ammo".to_string(),
                            power_id: 86,
                            power_progress: 1.0000,
                            power_selections: [
                                PowerSelectionPair { a: false, b: false },
                                PowerSelectionPair { a: false, b: false },
                                PowerSelectionPair { a: false, b: false },
                            ],
                            wheel_display_index: 0,
                            uses_talent_points: false,
                        },
                    ],
                    hotkeys: "".to_string(),
                    weapons: vec![7, 136],
                    weapon_mods: vec![
                        PlayerCharacterWeaponMod {
                            weapon_id: 136,
                            weapon_mod_ids: vec![36, 37],
                        },
                        PlayerCharacterWeaponMod {
                            weapon_id: 7,
                            weapon_mod_ids: vec![33],
                        },
                    ],
                    deployed: true,
                    leveled_up: true,
                },
            ),
        )];

        for (input, (expected_version, expected_dev_version, expected)) in test_data {
            let (version, dev_version, player_character) =
                PlayerCharacter::parse_input(input).unwrap();
            assert_eq!(&version, expected_version);
            assert_eq!(&dev_version, expected_dev_version);
            assert_eq!(&player_character, expected);
        }
    }
}
