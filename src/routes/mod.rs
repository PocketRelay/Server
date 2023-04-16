use crate::middleware::cors::cors_layer;
use axum::{middleware, Router};

mod auth;
mod content;
mod dashboard;
mod games;
mod gaw;
mod leaderboard;
mod players;
mod qos;
mod server;

/// Function for configuring the provided service config with all the
/// application routes.
///
/// `cfg`         Service config to configure
/// `token_store` The token store for token authentication
pub fn router() -> Router {
    Router::new()
        .nest("/content", content::router())
        .nest("/gaw", gaw::router())
        .nest("/qos", qos::router())
        .nest("/api", api_router())
        .nest("/", dashboard::router())
}

/// Creates a router for the routes that reside under /api
fn api_router() -> Router {
    Router::new()
        // Games routing
        .nest("/games", games::router())
        // Players routing
        .nest("/players", players::router())
        // Authentication routes
        .nest("/auth", auth::router())
        // Leaderboard routing
        .nest("/leaderboard", leaderboard::router())
        // Server details routes
        .nest("/server", server::router())
        // CORS middleware is applied to all API routes to allow browser access
        .layer(middleware::from_fn(cors_layer))
}
