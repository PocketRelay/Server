use tokio::join;

use crate::utils::jwt::Jwt;

use self::{
    game::{manager::GameManagerAddr, matchmaking::MatchmakingAddr},
    leaderboard::Leaderboard,
    retriever::Retriever,
};

pub mod game;
pub mod leaderboard;
pub mod retriever;

pub struct Services {
    pub game_manager: GameManagerAddr,
    pub matchmaking: MatchmakingAddr,
    pub leaderboard: Leaderboard,
    pub retriever: Option<Retriever>,
    pub jwt: Jwt,
}

impl Services {
    pub async fn init() -> Self {
        let (retriever, jwt) = join!(Retriever::new(), Jwt::new());
        let game_manager = GameManagerAddr::spawn();
        let matchmaking = MatchmakingAddr::spawn();
        let leaderboard = Leaderboard::default();

        Self {
            game_manager,
            matchmaking,
            leaderboard,
            retriever,
            jwt,
        }
    }
}
