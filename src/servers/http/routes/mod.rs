use super::{
    middleware::{cors::cors_layer, token::token_auth_layer},
    stores::token::TokenStore,
};
use crate::env;
use axum::{middleware, Router};

mod content;
mod games;
mod gaw;
mod leaderboard;
mod players;
mod qos;
mod server;
mod token;

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
}

/// Creates a router for the routes that reside under /api
fn api_router() -> Router {
    if env::from_env(env::API) {
        Router::new()
            // Games routing
            .nest("/games", games::router())
            // Players routing
            .nest("/players", players::router())
            // Apply the token auth middleware
            .layer(middleware::from_fn(token_auth_layer))
            // Routes that require token store access but arent protected
            .nest("/token", token::router())
            // Provide token store to API routes
            .layer(TokenStore::extension())
            // Non protected API routes
            .nest("/leaderboard", leaderboard::router())
    } else {
        // If the API is disable a default empty router is added
        Router::new()
    }
    // Even when the API is disabled the server route must still
    // be applied otherwise clients won't be able to check the server
    .nest("/server", server::router())
    // CORS middleware is applied to all API routes to allow browser access
    .layer(middleware::from_fn(cors_layer))
}
