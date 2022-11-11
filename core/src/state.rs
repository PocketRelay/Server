use std::sync::Arc;

use database::Database;
use tokio::sync::watch;

use crate::{
    env,
    game::{matchmaking::Matchmaking, Games},
    retriever::Retriever,
};

/// Global state that is shared throughout the application
pub struct GlobalState {
    pub games: Games,
    pub matchmaking: Matchmaking,
    pub db: Database,
    pub retriever: Option<Retriever>,
    pub shutdown: watch::Receiver<()>,
}

pub type GlobalStateArc = Arc<GlobalState>;

impl GlobalState {
    /// Initializes the global state with the provided shutdown
    /// reciever and returns it wrapped in an Arc
    pub async fn init(shutdown: watch::Receiver<()>) -> Arc<Self> {
        let db = {
            let file = env::str_env(env::DATABASE_FILE);
            Database::connect(file).await
        };

        let games = Games::new();
        let matchmaking = Matchmaking::new();
        let retriever = Retriever::new().await;

        let global_state = GlobalState {
            db,
            games,
            matchmaking,
            retriever,
            shutdown,
        };

        Arc::new(global_state)
    }
}
