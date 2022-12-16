/// Structure for an update to a player class
pub struct PlayerClassUpdate {
    /// The name of the class
    pub name: String,
    /// The current class level
    pub level: u8,
    /// The experience level of the class
    pub exp: f32,
    /// The number of promotions the class has
    pub promotions: u32,
}
