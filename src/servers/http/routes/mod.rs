use super::{
    middleware::{cors::cors_layer, token::guard_token_auth},
    stores::token::TokenStore,
};
use crate::env;
use axum::{middleware, Extension, Router};
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
pub fn router() -> Router {
    let mut router = Router::new();

    router = server::route(router);
    router = public::route(router);
    router = gaw::route(router);
    router = qos::route(router);

    // If the API is enabled
    if env::from_env(env::API) {
        let token_store = Arc::new(TokenStore::default());

        // Non protected API routes
        {
            router = leaderboard::route(router);
            router = token::route(router);
        }

        // Auth protected routes
        {
            let mut auth_router = Router::new();

            // Apply the underlying routes
            auth_router = games::route(auth_router);
            auth_router = players::route(auth_router);

            // Apply the token auth middleware
            auth_router = auth_router.layer(middleware::from_fn(guard_token_auth));

            // Merge the protected routes into the main router
            router = router.merge(auth_router);
        }

        // Token store is provided to all routes
        router = router.layer(Extension(token_store));
    }

    router.layer(middleware::from_fn(cors_layer))
}
