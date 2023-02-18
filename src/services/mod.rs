use self::{
    game::{manager::GameManager, matchmaking::MatchmakingLink},
    jwt::Jwt,
    leaderboard::LeaderboardLink,
    retriever::Retriever,
};
use interlink::prelude::Link;
use tokio::join;

pub mod game;
pub mod jwt;
pub mod leaderboard;
pub mod retriever;

pub struct Services {
    pub game_manager: Link<GameManager>,
    pub matchmaking: MatchmakingLink,
    pub leaderboard: LeaderboardLink,
    pub retriever: Option<Retriever>,
    pub jwt: Jwt,
}

impl Services {
    pub async fn init() -> Self {
        let (retriever, jwt) = join!(Retriever::new(), Jwt::new());
        let game_manager = GameManager::start();
        let matchmaking = MatchmakingLink::start();
        let leaderboard = LeaderboardLink::start();

        Self {
            game_manager,
            matchmaking,
            leaderboard,
            retriever,
            jwt,
        }
    }
}
