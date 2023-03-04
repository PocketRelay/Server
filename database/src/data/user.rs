use sea_orm::prelude::*;
use serde::{Deserialize, Serialize};

/// Enum for the different roles that a player could have used to
/// determine their permissions to access different server
/// functionality
#[derive(
    Deserialize, Serialize, Debug, Clone, PartialEq, PartialOrd, Ord, Eq, EnumIter, DeriveActiveEnum,
)]
#[sea_orm(rs_type = "u8", db_type = "TinyUnsigned")]
#[repr(u8)]
pub enum PlayerRole {
    /// The default no extra permissions level
    #[sea_orm(num_value = 0)]
    Default = 0,

    /// Administrator role which can be added and removed by
    /// super admin.
    #[sea_orm(num_value = 1)]
    Admin = 1,

    /// Super admin role which is created on startup and used to
    /// manage other user roles
    #[sea_orm(num_value = 2)]
    SuperAdmin = 2,
}
