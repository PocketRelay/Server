use blaze_pk::types::TdfMap;
use database::{players, DatabaseConnection, PlayersInterface};
use log::{debug, error};

use crate::{
    blaze::{
        components::{Authentication, Components, Util},
        errors::BlazeResult,
    },
    env,
};

use super::{
    shared::{OriginLoginReq, OriginLoginRes, UserSettingsAll},
    RetSession, Retriever,
};

/// Structure for the data return after retrieving the data
/// from an Origin account using the official servers.
pub struct OriginDetails {
    pub email: String,
    pub display_name: String,
}

impl Retriever {
    /// Async wrapper and enabled checker for fetching origin details from the
    /// official server using the provided origin auth token.
    pub async fn get_origin_player(
        &self,
        db: &DatabaseConnection,
        token: String,
    ) -> Option<players::Model> {
        if !env::from_env(env::ORIGIN_FETCH) {
            return None;
        }
        let mut session = self.session().await?;
        let details = session.get_origin_details(token).await?;

        let player = PlayersInterface::by_email(&db, &details.email, true)
            .await
            .ok()?;
        let player = match player {
            None => {
                let mut player = PlayersInterface::create(
                    &db,
                    details.email,
                    details.display_name,
                    String::new(),
                    true,
                )
                .await
                .ok()?;
                if env::from_env(env::ORIGIN_FETCH_DATA) {
                    match session.get_extra_data().await {
                        Some(values) => {
                            player = PlayersInterface::update_all(&db, player, values)
                                .await
                                .ok()?;
                        }
                        None => {
                            error!(
                                "Failed to fetch additional data for origin account (Name: {})",
                                &player.display_name
                            );
                        }
                    }
                }

                player
            }
            Some(player) => player,
        };
        Some(player)
    }
}

impl RetSession {
    /// Blocking implementation for retrieving the origin details from the official
    /// servers using the provided token will load the player settings if the
    /// PR_ORIGIN_FETCH_DATA env is enabled.
    async fn get_origin_details(&mut self, token: String) -> Option<OriginDetails> {
        let details = self.auth_origin(token).await.ok()?;
        debug!(
            "Retrieved origin details (Name: {}, Email: {})",
            &details.display_name, &details.email
        );
        Some(details)
    }

    /// Loads all the user data from UserSettingsLoadAll and sets the
    /// data on the origin details provided
    async fn get_extra_data(&mut self) -> Option<TdfMap<String, String>> {
        let value = self
            .request_empty::<UserSettingsAll>(Components::Util(Util::UserSettingsLoadAll))
            .await
            .ok()?;
        Some(value.value)
    }

    /// Authenticates with origin by sending the origin token and then
    /// returns the details from it with None as the data field
    async fn auth_origin(&mut self, token: String) -> BlazeResult<OriginDetails> {
        let value = self
            .request::<OriginLoginReq, OriginLoginRes>(
                Components::Authentication(Authentication::OriginLogin),
                &OriginLoginReq { token },
            )
            .await?;

        Ok(OriginDetails {
            email: value.email,
            display_name: value.display_name,
        })
    }
}
