pub mod galaxy_at_war;
pub mod player_data;
pub mod players;

pub type GalaxyAtWar = galaxy_at_war::Model;
pub type Player = players::Model;
pub type PlayerData = player_data::Model;
pub use players::PlayerRole;
