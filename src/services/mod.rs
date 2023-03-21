use crate::config::ServicesConfig;

use self::{
    game::manager::GameManager, leaderboard::Leaderboard, matchmaking::Matchmaking,
    retriever::Retriever, sessions::AuthedSessions, tokens::Tokens,
};
use interlink::prelude::Link;
use tokio::join;

pub mod game;
pub mod leaderboard;
pub mod matchmaking;
pub mod retriever;
pub mod sessions;
pub mod tokens;

pub struct Services {
    pub game_manager: Link<GameManager>,
    pub matchmaking: Link<Matchmaking>,
    pub leaderboard: Link<Leaderboard>,
    pub retriever: Option<Retriever>,
    pub sessions: Link<AuthedSessions>,
    pub tokens: Tokens,
}

impl Services {
    pub async fn init(config: ServicesConfig) -> Self {
        let (retriever, tokens) = join!(Retriever::new(config.retriever), Tokens::new());
        let game_manager = GameManager::start();
        let matchmaking = Matchmaking::start();
        let leaderboard = Leaderboard::start();
        let sessions = AuthedSessions::start();

        Self {
            game_manager,
            matchmaking,
            leaderboard,
            retriever,
            sessions,
            tokens,
        }
    }
}
