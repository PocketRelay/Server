use axum::{
    middleware,
    routing::{get, post, put},
    Router,
};

use crate::middleware::cors::cors_layer;

mod auth;
mod games;
mod gaw;
mod leaderboard;
mod players;
mod public;
mod qos;
mod server;

/// Function for configuring the provided service config with all the
/// application routes.
pub fn router() -> Router {
    Router::new()
        // Galaxy at war
        .route(
            "/authentication/sharedTokenLogin",
            get(gaw::shared_token_login),
        )
        .nest(
            "/galaxyatwar",
            Router::new()
                .route("/getRatings/:id", get(gaw::get_ratings))
                .route("/increaseRatings/:id", get(gaw::increase_ratings)),
        )
        // Quality of service
        .route("/qos/qos", get(qos::qos))
        // Dashboard API
        .nest(
            "/api",
            Router::new()
                // Games routing
                .nest(
                    "/games",
                    Router::new()
                        .route("/", get(games::get_games))
                        .route("/:id", get(games::get_game)),
                )
                // Players routing
                .nest(
                    "/players",
                    Router::new()
                        .route("/", get(players::get_players))
                        .route("/self", get(players::get_self).delete(players::delete_self))
                        .route("/self/password", put(players::update_password))
                        .route("/self/details", put(players::update_details))
                        .route(
                            "/:id",
                            get(players::get_player).delete(players::delete_player),
                        )
                        .route("/:id/data", get(players::all_data))
                        .route(
                            "/:id/data/:key",
                            get(players::get_data)
                                .put(players::set_data)
                                .delete(players::delete_data),
                        )
                        .route("/:id/galaxy_at_war", get(players::get_player_gaw))
                        .route("/:id/password", put(players::set_password))
                        .route("/:id/details", put(players::set_details))
                        .route("/:id/role", put(players::set_role)),
                )
                // Authentication routes
                .nest(
                    "/auth",
                    Router::new()
                        .route("/login", post(auth::login))
                        .route("/create", post(auth::create)),
                )
                // Leaderboard routing
                .nest(
                    "/leaderboard",
                    Router::new()
                        .route("/:name", get(leaderboard::get_leaderboard))
                        .route("/:name/:player_id", get(leaderboard::get_player_ranking)),
                )
                // Server details routes
                .nest(
                    "/server",
                    Router::new()
                        .route("/", get(server::server_details))
                        .route("/log", get(server::get_log))
                        .route("/upgrade", get(server::upgrade))
                        .route("/telemetry", post(server::submit_telemetry))
                        .route("/dashboard", get(server::dashboard_details)),
                )
                .layer(middleware::from_fn(cors_layer)),
        )
        // Public content fallback
        .fallback_service(public::PublicContent)
}
