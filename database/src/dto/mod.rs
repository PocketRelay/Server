//! This module contains modules for intermediate structures that
//! are used to group data that is being passed to database functions
//! from things such as HTTP routes
pub mod player_characters;
pub mod player_classes;
pub mod players;

pub enum ParsedUpdate {
    Class(u16, player_classes::PlayerClassUpdate),
    Character(u16, player_characters::PlayerCharacterUpdate),
    Data(players::PlayerDataUpdate),
}
