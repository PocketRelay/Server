use self::{
    game::manager::GameManager, leaderboard::Leaderboard, matchmaking::Matchmaking,
    retriever::Retriever, tokens::Tokens,
};
use interlink::prelude::Link;
use tokio::join;

pub mod game;
pub mod leaderboard;
pub mod matchmaking;
pub mod retriever;
pub mod tokens;

pub struct Services {
    pub game_manager: Link<GameManager>,
    pub matchmaking: Link<Matchmaking>,
    pub leaderboard: Link<Leaderboard>,
    pub retriever: Option<Retriever>,
    pub tokens: Tokens,
}

impl Services {
    pub async fn init() -> Self {
        let (retriever, tokens) = join!(Retriever::new(), Tokens::new());
        let game_manager = GameManager::start();
        let matchmaking = Matchmaking::start();
        let leaderboard = Leaderboard::start();

        Self {
            game_manager,
            matchmaking,
            leaderboard,
            retriever,
            tokens,
        }
    }
}
