pub(crate) mod galaxy_at_war;
pub(crate) mod player_characters;
pub(crate) mod player_classes;
pub(crate) mod players;

pub type GalaxyAtWar = galaxy_at_war::Model;
pub type PlayerClass = player_classes::Model;
pub type PlayerCharacter = player_characters::Model;
pub type Player = players::Model;
