//! Retriever service for completing the Origin account authentication
//! and data loading flow

use super::{models::OriginLoginResponse, OfficialSession, RetrieverResult};
use crate::{
    config::Config,
    database::entities::{Player, PlayerData, PlayerRole},
    session::models::{auth::OriginLoginRequest, util::SettingsResponse},
    utils::{
        components::{authentication, util},
        hashing::hash_password,
    },
};
use log::{debug, error, warn};
use sea_orm::{DatabaseConnection, DbErr};
use tdf::TdfMap;
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
        config: &Config,
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

        let mut role = PlayerRole::Default;
        let mut password: Option<String> = None;

        // If there is a super admin defined
        if config.dashboard.is_super_email(&details.email) {
            // Use the super admin role
            role = PlayerRole::SuperAdmin;

            // Update the password with the specified one
            if let Some(super_password) = config.dashboard.super_password.as_ref() {
                if !super_password.is_empty() {
                    let password_hash =
                        hash_password(super_password).expect("Failed to hash super user password");
                    password = Some(password_hash);
                }
            }
        }

        let player: Player =
            Player::create(db, details.email, details.display_name, password, role).await?;

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
                authentication::COMPONENT,
                authentication::ORIGIN_LOGIN,
                OriginLoginRequest { token, ty: 1 },
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
            .request_empty::<SettingsResponse>(util::COMPONENT, util::USER_SETTINGS_LOAD_ALL)
            .await?;
        Ok(value.settings)
    }
}
