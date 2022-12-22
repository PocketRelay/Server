/// Structure for updating players using the HTTP interface with the
/// optional JSON fields
pub struct PlayerUpdate {
    /// Optional email address for changing the email field
    pub email: Option<String>,
    /// Optional display name for changing the display name
    pub display_name: Option<String>,
    /// Optional origin value for changing the origin state
    pub origin: Option<bool>,
    /// Optional password which has already been hashed in the HTTP layer
    pub password: Option<String>,
    /// Optional new credits amount
    pub credits: Option<u32>,
    /// Optional inventory string
    pub inventory: Option<String>,
    /// Optional reward value
    pub csreward: Option<u16>,
}

/// Structure for an update to the base data of a player that was
/// parsed from an ME3 string
pub struct PlayerBaseUpdate {
    /// The number of credits the player has
    pub credits: u32,
    /// The number of credits the player has spent
    pub credits_spent: u32,
    /// The number of games played by the player
    pub games_played: u32,
    /// The number of seconds played by the player
    pub seconds_played: u32,
    /// The encoded player inventory string
    pub inventory: String,
}
