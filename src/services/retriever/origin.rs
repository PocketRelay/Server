use super::{
    models::{OriginLoginRequest, OriginLoginResponse, SettingsResponse},
    RetSession, Retriever,
};
use crate::utils::components::{Authentication, Components, Util};
use blaze_pk::types::TdfMap;
use log::debug;

/// Service for providing origin flows from a retriever
/// instance if available
pub struct OriginFlowService {
    /// Whether data fetching is enabled within the
    /// created origin flows
    pub data: bool,
}

impl OriginFlowService {
    /// Creates a new origin flow from the provided retriever
    ///
    /// `retriever` The retriever to use to create the session for the flow
    pub async fn create(&self, retriever: &Retriever) -> Option<OriginFlow> {
        let session = retriever.session().await?;
        Some(OriginFlow {
            session,
            data: self.data,
        })
    }
}

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
    pub data: bool,
}

impl OriginFlow {
    /// Authenticates with the official servers using the provided token. Will
    /// return Origin details if the authentication process went without error
    ///
    /// `token` The token to authenticate with
    pub async fn authenticate(&mut self, token: &str) -> Option<OriginDetails> {
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
