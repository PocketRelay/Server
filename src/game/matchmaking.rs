use std::collections::HashMap;

use super::enums::{Difficulty, EnemyType};

#[derive(Debug)]
pub enum MatchRules {
    Map,
    Enemy(EnemyType),
    Difficulty(Difficulty),
    Other(String),
}

pub struct RuleSet {
    values: Vec<MatchRules>,
}
