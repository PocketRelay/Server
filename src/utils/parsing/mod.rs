//! Utilities for parsing ME3 strings
use serde::Serialize;
use std::str::{FromStr, Split};

pub mod parser;
pub mod player_character;

/// Parser for parsing strings that are formatted using the ME3
/// string format. For this format the values are split by a ;
/// and the first two values indicate the version
///
/// VERSION1;VERSION2;DATA1;DATA2;
/// 20;4;Sentinel;20;0.00000;50
struct MEParser<'a>(Split<'a, char>);

impl<'a> MEParser<'a> {
    pub fn new(value: &'a str) -> Option<MEParser<'a>> {
        let mut split = value.split(';');

        // Consume the version portion
        let _v1 = split.next()?;
        let _v2 = split.next()?;

        Some(MEParser(split))
    }

    #[inline]
    pub fn next(&mut self) -> Option<&'a str> {
        self.0.next()
    }

    pub fn parse_next<F: FromStr>(&mut self) -> Option<F> {
        let next = self.next()?;
        next.parse::<F>().ok()
    }

    pub fn skip(&mut self, n: usize) -> Option<()> {
        for _ in 0..n {
            self.next()?;
        }
        Some(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlayerClass<'a> {
    /// The class name
    pub name: &'a str,
    /// The class level
    pub level: u8,
    // The amount of exp the class has (Field ignored for parsing)
    // pub exp: f32,
    /// The number of promotions the class has
    pub promotions: u32,
}

impl PlayerClass<'_> {
    /// Attempts to parse the provided player class data string
    ///
    /// # Format
    /// ```
    /// 20;4;Adept;20;0;50
    /// 20;4;NAME;LEVEL;EXP;PROMOTIONS
    /// ```
    pub fn parse(value: &str) -> Option<PlayerClass<'_>> {
        let mut parser = MEParser::new(value)?;
        let name = parser.next()?;
        let level = parser.parse_next()?;
        parser.skip(1)?;
        let promotions = parser.parse_next()?;
        Some(PlayerClass {
            name,
            level,
            promotions,
        })
    }
}

// Unused full format declaration for the player character data
//
// /// Structure for a player character model stored in the database
// #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
// pub struct PlayerCharacter<'a> {
//     /// The name of the character kit contains the name of the class
//     pub kit_name: &'a str,
//     /// The name given to this character by the player
//     pub name: &'a str,
//     pub tint1: u16,
//     pub tint2: u16,
//     pub pattern: u16,
//     pub pattern_color: u16,
//     pub phong: u16,
//     pub emissive: u16,
//     pub skin_tone: u16,
//     /// The total number of seconds played as this character
//     pub seconds_played: u32,
//     pub timestamp_year: u32,
//     pub timestamp_month: u32,
//     pub timestamp_day: u32,
//     pub timestamp_seconds: u32,
//     /// Powers configuration string
//     ///
//     /// Name
//     /// Unlocked rank 0 - 6
//     /// (1 if first split A is unlocked or 0 if not)
//     /// (1 if first split B is unlocked or 0 if not)
//     /// (2 if second split A is unlocked or 0 if not)
//     /// (2 if second split B is unlocked or 0 if not)
//     /// (3 if third split A is unlocked or 0 if not)
//     /// (3 if third split B is unlocked or 0 if not)
//     /// Unknown 0 - 6
//     /// Character specific flag? True/False
//     ///
//     /// # Examples
//     /// ```
//     /// AdrenalineRush 139 6.0000 1 0 2 0 3 0 0 True,
//     /// ConcussiveShot 148 6.0000 1 0 0 2 0 3 5 True,
//     /// FragGrenade 159 0.0000 0 0 0 0 0 0 2 True,
//     /// MPPassive 206 6.0000 0 1 2 0 0 3 5 True,
//     /// MPMeleePassive 204 6.0000 0 1 0 2 0 3 5 True,
//     /// ```
//     ///
//     /// ```
//     /// # Standard abilities from mp
//     /// Consumable_Rocket 88 1.0000 0 0 0 0 0 0 3 False,
//     /// Consumable_Revive 87 1.0000 0 0 0 0 0 0 4 False,
//     /// Consumable_Shield 89 1.0000 0 0 0 0 0 0 5 False,
//     /// Consumable_Ammo 86 1.0000 0 0 0 0 0 0 6 False
//     /// ```
//     pub powers: &'a str,
//     /// Hotkey configuration string
//     pub hotkeys: &'a str,
//     /// Weapon configuration string
//     /// List of weapon IDs should not be more than two
//     /// 135,25
//     pub weapons: &'a str,
//     /// Weapon mod configuration string
//     /// List of weapon mods split by spaces for each
//     /// gun. Can contain 1 or 2
//     /// 135 34,25 47
//     pub weapon_mods: &'a str,
//     /// Whether this character has been deployed before
//     /// (Aka used)
//     pub deployed: bool,
//     /// Whether this character has leveled up
//     pub leveled_up: bool,
// }
