use super::{
    middleware::{cors::CorsLayer, token::TokenAuthLayer},
    stores::token::TokenStore,
};
use crate::env;
use axum::Router;
use std::sync::Arc;

mod games;
mod gaw;
mod leaderboard;
mod players;
mod public;
mod qos;
mod server;
mod token;

/// Function for configuring the provided service config with all the
/// application routes.
///
/// `cfg`         Service config to configure
/// `token_store` The token store for token authentication
pub fn route(router: &mut Router) {
    router.layer(CorsLayer);

    server::route(router);
    public::route(router);
    gaw::route(router);
    qos::route(router);

    // If the API is enabled
    if env::from_env(env::API) {
        let token_store = Arc::new(TokenStore::default());

        // Non protected API routes
        {
            leaderboard::route(router);
        }

        // Routes with token store provided
        {
            let mut token_router = Router::new();
            token::route(&mut token_router);

            // Add token store state
            token_router.with_state(token_store.clone());

            // Merge the token auth routes into the main router
            router.merge(token_router);
        }

        // Auth protected routes
        {
            let mut auth_router = Router::new();

            // Apply the underlying routes
            games::route(&mut auth_router);
            players::route(&mut auth_router);

            // Apply the token auth layer
            auth_router.layer(TokenAuthLayer::new(token_store));

            // Merge the protected routes into the main router
            router.merge(auth_router);
        }
    }
}
