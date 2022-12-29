use crate::blaze::components::Components;
use blaze_pk::router::Router;

use super::session::SessionAddr;

mod auth;
mod game_manager;
mod messaging;
mod other;
mod stats;
mod user_sessions;
mod util;

/// Function which creates a router for sessions to use
pub fn router() -> Router<Components, SessionAddr> {
    let mut router = Router::new();
    auth::route(&mut router);
    game_manager::route(&mut router);
    stats::route(&mut router);
    util::route(&mut router);
    messaging::route(&mut router);
    user_sessions::route(&mut router);
    other::route(&mut router);
    router
}
