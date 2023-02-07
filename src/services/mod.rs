use self::{
    game::{manager::GameManagerAddr, matchmaking::MatchmakingAddr},
    jwt::Jwt,
    leaderboard::LeaderboardAddr,
    retriever::Retriever,
};
use tokio::join;

pub mod game;
pub mod jwt;
pub mod leaderboard;
pub mod retriever;

pub struct Services {
    pub game_manager: GameManagerAddr,
    pub matchmaking: MatchmakingAddr,
    pub leaderboard: LeaderboardAddr,
    pub retriever: Option<Retriever>,
    pub jwt: Jwt,
}

impl Services {
    pub async fn init() -> Self {
        let (retriever, jwt) = join!(Retriever::new(), Jwt::new());
        let game_manager = GameManagerAddr::spawn();
        let matchmaking = MatchmakingAddr::spawn();
        let leaderboard = LeaderboardAddr::spawn();

        Self {
            game_manager,
            matchmaking,
            leaderboard,
            retriever,
            jwt,
        }
    }
}
