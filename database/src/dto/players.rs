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
