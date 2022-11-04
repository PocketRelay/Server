use blaze_pk::TdfMap;
use log::{debug, error};
use tokio::task::spawn_blocking;

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
    pub data: Option<TdfMap<String, String>>,
}

impl Retriever {
    /// Async wrapper and enabled checker for fetching origin details from the
    /// official server using the provided origin auth token.
    pub async fn get_origin_details(&self, token: String) -> Option<OriginDetails> {
        if !env::bool_env(env::ORIGIN_FETCH) {
            return None;
        }
        let mut session = self.session()?;
        spawn_blocking(move || session.get_origin_details(token))
            .await
            .ok()?
    }
}

impl RetSession {
    /// Blocking implementation for retrieving the origin details from the official
    /// servers using the provided token will load the player settings if the
    /// PR_ORIGIN_FETCH_DATA env is enabled.
    fn get_origin_details(&mut self, token: String) -> Option<OriginDetails> {
        let mut details = self.auth_origin(token).ok()?;
        debug!(
            "Retrieved origin details (Name: {}, Email: {})",
            &details.display_name, &details.email
        );
        if env::bool_env(env::ORIGIN_FETCH_DATA) {
            if let Err(err) = self.get_extra_data(&mut details) {
                error!(
                    "Failed to fetch additional data for origin account (Name: {})\n{:?}",
                    &details.display_name, err
                );
            }
        }
        Some(details)
    }

    /// Loads all the user data from UserSettingsLoadAll and sets the
    /// data on the origin details provided
    fn get_extra_data(&mut self, details: &mut OriginDetails) -> BlazeResult<()> {
        let value =
            self.request_empty::<UserSettingsAll>(Components::Util(Util::UserSettingsLoadAll))?;
        details.data = Some(value.value);
        Ok(())
    }

    /// Authenticates with origin by sending the origin token and then
    /// returns the details from it with None as the data field
    fn auth_origin(&mut self, token: String) -> BlazeResult<OriginDetails> {
        let value = self.request::<OriginLoginReq, OriginLoginRes>(
            Components::Authentication(Authentication::OriginLogin),
            &OriginLoginReq { token },
        )?;

        Ok(OriginDetails {
            email: value.email,
            display_name: value.display_name,
            data: None,
        })
    }
}
