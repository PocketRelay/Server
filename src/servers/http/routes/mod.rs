use super::{
    middleware::{cors::cors_layer, token::token_auth_layer},
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

        let mut api_router = Router::new();

        {
            // Auth protected routes
            {
                api_router = games::route(api_router);
                api_router = players::route(api_router);
                // Apply the token auth middleware
                api_router = api_router.layer(middleware::from_fn(token_auth_layer));
            }

            // Routes that require token store access but arent protected
            {
                api_router = token::route(api_router);
            }

            // Provide token store to API routes
            api_router = api_router.layer(Extension(token_store));
        }

        // Non protected API routes
        {
            api_router = leaderboard::route(api_router);
        }

        router = router.merge(api_router);
    }

    // CORS middleware is applied to all routes to allow browser access
    router.layer(middleware::from_fn(cors_layer))
}
