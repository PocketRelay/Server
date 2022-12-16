//! Utilites for parsing ME3 strings
use database::dto::{
    player_characters::PlayerCharacterUpdate,
    player_classes::PlayerClassUpdate,
    players::{PlayerBaseUpdate, PlayerDataUpdate},
    ParsedUpdate,
};
use log::warn;
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

pub fn parse_update(key: String, value: String) -> Option<ParsedUpdate> {
    if key.starts_with("class") {
        let index = match parse_index_key(&key, "class") {
            Ok(value) => value,
            Err(err) => {
                warn!("Unable to parse player class index key: {err:?}");
                return None;
            }
        };
        let value = parse_player_class(value)?;
        Some(ParsedUpdate::Class(index, value))
    } else if key.starts_with("char") {
        let index = match parse_index_key(&key, "char") {
            Ok(value) => value,
            Err(err) => {
                warn!("Unable to parse player character index key: {err:?}");
                return None;
            }
        };
        let value = parse_player_character(value)?;
        Some(ParsedUpdate::Character(index, value))
    } else {
        let value = parse_player_update(key, value)?;
        Some(ParsedUpdate::Data(value))
    }
}

pub fn parse_updates(values: impl Iterator<Item = (String, String)>) -> Vec<ParsedUpdate> {
    values
        .filter_map(|(key, value)| parse_update(key, value))
        .collect()
}

pub fn parse_player_update(key: String, value: String) -> Option<PlayerDataUpdate> {
    Some(match &key as &str {
        "Base" => {
            let value = parse_player_base(value)?;
            PlayerDataUpdate::Base(value)
        }
        "FaceCodes" => PlayerDataUpdate::FaceCodes(value),
        "NewItem" => PlayerDataUpdate::NewItem(value),
        "csreward" => {
            let value: u16 = value.parse().unwrap_or(0);
            PlayerDataUpdate::ChallengeReward(value)
        }
        "Completion" => PlayerDataUpdate::Completion(value),
        "Progress" => PlayerDataUpdate::Progress(value),
        "cscompletion" => PlayerDataUpdate::Cscompletion(value),
        "cstimestamps" => PlayerDataUpdate::Cstimestamps(value),
        "cstimestamps2" => PlayerDataUpdate::Cstimestamps2(value),
        "cstimestamps3" => PlayerDataUpdate::Cstimestamps3(value),
        _ => return None,
    })
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

#[derive(Debug)]
pub enum IndexKeyError {
    InvalidKey,
    InvalidIndex,
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
pub fn parse_player_class(value: String) -> Option<PlayerClassUpdate> {
    let mut parser = MEStringParser::new(&value)?;
    let name = parser.next_str()?;
    let level = parser.parse_next()?;
    let exp = parser.parse_next()?;
    let promotions = parser.parse_next()?;
    Some(PlayerClassUpdate {
        name,
        level,
        exp,
        promotions,
    })
}

pub fn parse_index_key(key: &str, prefix: &str) -> Result<u16, IndexKeyError> {
    key.strip_prefix(prefix)
        .ok_or(IndexKeyError::InvalidKey)?
        .parse()
        .map_err(|_| IndexKeyError::InvalidIndex)
}

pub fn parse_player_character(value: String) -> Option<PlayerCharacterUpdate> {
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
    Some(PlayerCharacterUpdate {
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
