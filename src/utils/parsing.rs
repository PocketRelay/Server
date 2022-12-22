//! Utilites for parsing ME3 strings
use database::dto::players::PlayerBaseUpdate;
use serde::Serialize;
use std::str::{FromStr, Split};

/// Structure for parsing ME3 format strings which are strings made up of sets
/// of data split by ; that each start with the 20;4;
///
/// # Example
/// ```20;4;Sentinel;20;0.0000;50```
pub struct MEStringParser<'a> {
    split: Split<'a, char>,
}

impl<'a> MEStringParser<'a> {
    pub fn new(value: &'a str) -> Option<MEStringParser<'a>> {
        if !value.starts_with("20;4;") {
            return None;
        }
        let split = value[5..].split(';');
        Some(MEStringParser { split })
    }

    pub fn skip(&mut self, count: usize) -> Option<()> {
        for _ in 0..count {
            self.split.next()?;
        }
        Some(())
    }

    pub fn next_str(&mut self) -> Option<String> {
        let next = self.split.next()?;
        Some(next.to_string())
    }

    pub fn parse_next<F: FromStr>(&mut self) -> Option<F> {
        let next = self.split.next()?;
        next.parse::<F>().ok()
    }

    pub fn next_bool(&mut self) -> Option<bool> {
        let next = self.split.next()?;
        if next == "True" {
            Some(true)
        } else if next == "False" {
            Some(false)
        } else {
            None
        }
    }
}

/// Attempts to parse the provided player base data string and update the fields
/// on the provided active player let  Will return a None option if parsing
/// failed.
///
/// # Format
/// ```
/// 20;4;21474;-1;0;0;0;50;180000;0;fff....(LARGE SEQUENCE OF INVENTORY CHARS)
/// 20;4;CREDITS;UNKNOWN;UKNOWN;CREDITS_SPENT;UKNOWN;GAMES_PLAYED;SECONDS_PLAYED;UKNOWN;INVENTORY
/// ```
///
/// `value` The value to parse
pub fn parse_player_base(value: String) -> Option<PlayerBaseUpdate> {
    let mut parser = MEStringParser::new(&value)?;
    let credits: u32 = parser.parse_next()?;
    parser.skip(2)?;
    let credits_spent: u32 = parser.parse_next()?;
    parser.skip(1)?;
    let games_played: u32 = parser.parse_next()?;
    let seconds_played: u32 = parser.parse_next()?;
    parser.skip(1)?;
    let inventory = parser.next_str()?;
    Some(PlayerBaseUpdate {
        credits,
        credits_spent,
        games_played,
        seconds_played,
        inventory,
    })
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlayerClass {
    /// The class name
    pub name: String,
    /// The class level
    pub level: u8,
    /// The amount of exp the class has
    pub exp: f32,
    /// The number of promotions the class has
    pub promotions: u32,
}

/// Attempts to parse the provided player character data string and update the fields
/// on the provided active player character model. Will return a None option if parsing
/// failed.
///
/// # Format
/// ```
/// 20;4;Adept;20;0;50
/// 20;4;NAME;LEVEL;EXP;PROMOTIONS
/// ```
pub fn parse_player_class(value: String) -> Option<PlayerClass> {
    let mut parser = MEStringParser::new(&value)?;
    let name = parser.next_str()?;
    let level = parser.parse_next()?;
    let exp = parser.parse_next()?;
    let promotions = parser.parse_next()?;
    Some(PlayerClass {
        name,
        level,
        exp,
        promotions,
    })
}

/// Structure for a player character model stored in the database
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PlayerCharacter {
    /// The name of the character kit contains the name of the class
    pub kit_name: String,
    /// The name given to this character by the player
    pub name: String,
    pub tint1: u16,
    pub tint2: u16,
    pub pattern: u16,
    pub pattern_color: u16,
    pub phong: u16,
    pub emissive: u16,
    pub skin_tone: u16,
    /// The total number of seconds played as this character
    pub seconds_played: u32,
    pub timestamp_year: u32,
    pub timestamp_month: u32,
    pub timestamp_day: u32,
    pub timestamp_seconds: u32,
    /// Powers configuration string
    ///
    /// Name
    /// Unlocked rank 0 - 6
    /// (1 if first split A is unlocked or 0 if not)
    /// (1 if first split B is unlocked or 0 if not)
    /// (2 if second split A is unlocked or 0 if not)
    /// (2 if second split B is unlocked or 0 if not)
    /// (3 if third split A is unlocked or 0 if not)
    /// (3 if third split B is unlocked or 0 if not)
    /// Unknown 0 - 6
    /// Charcter specific flag? True/False
    ///
    /// # Examples
    /// ```
    /// AdrenalineRush 139 6.0000 1 0 2 0 3 0 0 True,
    /// ConcussiveShot 148 6.0000 1 0 0 2 0 3 5 True,
    /// FragGrenade 159 0.0000 0 0 0 0 0 0 2 True,
    /// MPPassive 206 6.0000 0 1 2 0 0 3 5 True,
    /// MPMeleePassive 204 6.0000 0 1 0 2 0 3 5 True,
    /// ```
    ///
    /// ```
    /// # Standard abilities from mp
    /// Consumable_Rocket 88 1.0000 0 0 0 0 0 0 3 False,
    /// Consumable_Revive 87 1.0000 0 0 0 0 0 0 4 False,
    /// Consumable_Shield 89 1.0000 0 0 0 0 0 0 5 False,
    /// Consumable_Ammo 86 1.0000 0 0 0 0 0 0 6 False
    /// ```
    pub powers: String,
    /// Hotkey configuration string
    pub hotkeys: String,
    /// Weapon configuration string
    /// List of weapon IDs should not be more than two
    /// 135,25
    pub weapons: String,
    /// Weapon mod configuration string
    /// List of weapon mods split by spaces for each
    /// gun. Can contain 1 or 2
    /// 135 34,25 47
    pub weapon_mods: String,
    /// Whether this character has been deployed before
    /// (Aka used)
    pub deployed: bool,
    /// Whether this character has leveled up
    pub leveled_up: bool,
}

pub fn parse_player_character(value: String) -> Option<PlayerCharacter> {
    let mut parser = MEStringParser::new(&value)?;
    let kit_name: String = parser.next_str()?;
    let name: String = parser.parse_next()?;
    let tint1: u16 = parser.parse_next()?;
    let tint2: u16 = parser.parse_next()?;
    let pattern: u16 = parser.parse_next()?;
    let pattern_color: u16 = parser.parse_next()?;
    let phong: u16 = parser.parse_next()?;
    let emissive: u16 = parser.parse_next()?;
    let skin_tone: u16 = parser.parse_next()?;
    let seconds_played: u32 = parser.parse_next()?;
    let timestamp_year: u32 = parser.parse_next()?;
    let timestamp_month: u32 = parser.parse_next()?;
    let timestamp_day = parser.parse_next()?;
    let timestamp_seconds: u32 = parser.parse_next()?;
    let powers: String = parser.next_str()?;
    let hotkeys: String = parser.next_str()?;
    let weapons: String = parser.next_str()?;
    let weapon_mods: String = parser.next_str()?;
    let deployed: bool = parser.next_bool()?;
    let leveled_up: bool = parser.next_bool()?;
    Some(PlayerCharacter {
        kit_name,
        name,
        tint1,
        tint2,
        pattern,
        pattern_color,
        phong,
        emissive,
        skin_tone,
        seconds_played,
        timestamp_year,
        timestamp_month,
        timestamp_day,
        timestamp_seconds,
        powers,
        hotkeys,
        weapons,
        weapon_mods,
        deployed,
        leveled_up,
    })
}

#[cfg(test)]
mod test {
    use crate::utils::parsing::MEStringParser;

    #[test]
    fn test_a() {
        let value = "20;4;AABB;123;DWADA";
        let mut parser = MEStringParser::new(value).unwrap();
        assert_eq!(parser.next_str().unwrap(), "AABB");
        assert_eq!(parser.parse_next::<u16>().unwrap(), 123);
        assert_eq!(parser.next_str().unwrap(), "DWADA");
    }
}
