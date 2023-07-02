//! Retriever service for completing the Origin account authentication
//! and data loading flow

use super::{models::OriginLoginResponse, OfficialSession, RetrieverResult};
use crate::{
    database::entities::{Player, PlayerData},
    session::models::{auth::OriginLoginRequest, util::SettingsResponse},
    utils::components::{Authentication, Components, Util},
};
use blaze_pk::types::TdfMap;
use log::{debug, error, warn};
use sea_orm::{DatabaseConnection, DbErr};
use thiserror::Error;

/// Service for providing origin flows from a retriever
/// instance if available
///
/// Fallback for if official servers get shutdown. Try using data from
/// https://service-aggregation-layer.juno.ea.com/graphql?operationName=GetUserPlayer&variables=%7B%7D&extensions=%7B%22persistedQuery%22%3A%7B%22version%22%3A1%2C%22sha256Hash%22%3A%22387cef4a793043a4c76c92ff4f2bceb7b25c3438f9c3c4fd5eb67eea18272657%22%7D%7D
/// to create origin authentication
pub struct OriginFlowService {
    /// Whether data fetching is enabled within the
    /// created origin flows
    pub data: bool,
}

impl OriginFlowService {
    /// Creates a new origin flow from the provided retriever
    ///
    /// `session` The session to use for the flow
    pub fn create(&self, session: OfficialSession) -> OriginFlow {
        OriginFlow {
            session,
            data: self.data,
        }
    }
}

/// Flow structure for complete authentication through
/// origin and optionally loading the player data
pub struct OriginFlow {
    /// The session to the official server
    session: OfficialSession,
    /// Whether to load the origin account data
    data: bool,
}

#[derive(Debug, Error)]
pub enum OriginError {
    /// Failed to complete the authentication process
    #[error("Failed to authenticate account with official servers")]
    FailedAuthenticate,
    /// Database error occurred
    #[error(transparent)]
    Database(#[from] DbErr),
}

impl OriginFlow {
    /// Attempts to login to the Origin account associated to the provided `token`
    /// then searches for the account details locally, creating the account if
    /// it doesn't exist
    pub async fn login(
        &mut self,
        db: &DatabaseConnection,
        token: String,
    ) -> Result<Player, OriginError> {
        // Authenticate with the official servers
        let details = self
            .authenticate(token)
            .await
            .map_err(|_| OriginError::FailedAuthenticate)?;

        // Check if the account with that email already exists
        if let Some(player) = Player::by_email(db, &details.email).await? {
            return Ok(player);
        }

        let player: Player = Player::create(db, details.email, details.display_name, None).await?;

        // If data fetching is ena
        if self.data {
            if let Ok(settings) = self.get_settings().await {
                debug!("Loaded player data from official server");
                PlayerData::set_bulk(db, player.id, settings.into_iter()).await?;
            } else {
                warn!(
                    "Unable to load origin player settings from official servers (Name: {}, Email: {})",
                    &player.display_name, &player.email
                );
            }
        }

        Ok(player)
    }

    /// Authenticates with the official servers using the provided `token`. Will
    /// return Origin details if the authentication process went without error
    async fn authenticate(&mut self, token: String) -> RetrieverResult<OriginLoginResponse> {
        let value = self
            .session
            .request::<OriginLoginRequest, OriginLoginResponse>(
                Components::Authentication(Authentication::OriginLogin),
                OriginLoginRequest { token },
            )
            .await?;

        debug!(
            "Retrieved Origin details (Name: {}, Email: {})",
            &value.display_name, &value.email
        );
        Ok(value)
    }

    /// Loads the user settings from the official server. Must be called after
    /// authenticate or it will thrown an error.
    async fn get_settings(&mut self) -> RetrieverResult<TdfMap<String, String>> {
        let value = self
            .session
            .request_empty::<SettingsResponse>(Components::Util(Util::UserSettingsLoadAll))
            .await?;
        Ok(value.settings)
    }
}
