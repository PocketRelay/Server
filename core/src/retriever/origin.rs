use blaze_pk::types::TdfMap;

use log::debug;

use crate::blaze::components::{Authentication, Components, Util};

use super::{
    codec::{OriginLoginRequest, OriginLoginResponse, SettingsResponse},
    RetSession, Retriever,
};

/// Structure for the data return after retrieving the data
/// from an Origin account using the official servers.
pub struct OriginDetails {
    pub email: String,
    pub display_name: String,
}

/// Flow structure for complete authentication through
/// origin and optionally loading the player data
pub struct OriginFlow {
    session: RetSession,
}

impl OriginFlow {
    /// Authenticates with the official servers using the provided token. Will
    /// return Origin details if the authentication process went without error
    ///
    /// `token` The token to authenticate with
    pub async fn authenticate(&mut self, token: String) -> Option<OriginDetails> {
        let value = self
            .session
            .request::<OriginLoginRequest, OriginLoginResponse>(
                Components::Authentication(Authentication::OriginLogin),
                OriginLoginRequest { token },
            )
            .await
            .ok()?;

        let details = OriginDetails {
            email: value.email,
            display_name: value.display_name,
        };

        debug!(
            "Retrieved origin details (Name: {}, Email: {})",
            &details.display_name, &details.email
        );
        Some(details)
    }

    /// Loads the user settings from the official server. Must be called after
    /// authenticate or it will thrown an error.
    pub async fn get_settings(&mut self) -> Option<TdfMap<String, String>> {
        let value = self
            .session
            .request_empty::<SettingsResponse>(Components::Util(Util::UserSettingsLoadAll))
            .await
            .ok()?;
        Some(value.settings)
    }
}

impl Retriever {
    /// Creates a new origin flow
    pub async fn create_origin_flow(&self) -> Option<OriginFlow> {
        let session = self.session().await?;
        Some(OriginFlow { session })
    }
}
