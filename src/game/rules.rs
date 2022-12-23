use super::AttrMap;

/// Rulesets are fairly cheap to clone. Rule values are not usually
/// very long.
#[derive(Clone)]
pub struct RuleSet {
    /// Map rule provided in the matchmaking request
    map_rule: Option<String>,
    /// Enemy rule provided in the matchmaking request
    enemy_rule: Option<String>,
    /// Difficulty rule provided in the matchmaking request
    difficulty_rule: Option<String>,
}

impl RuleSet {
    /// Attribute determining the game privacy for public
    /// match checking
    const PRIVACY_ATTR: &str = "ME3privacy";

    /// Map attribute and rule keys
    const MAP_ATTR: &str = "ME3map";
    const MAP_RULE: &str = "ME3_gameMapMatchRule";

    /// Enemy attribute and rule keys
    const ENEMY_ATTR: &str = "ME3gameEnemyType";
    const ENEMY_RULE: &str = "ME3_gameEnemyTypeRule";

    /// Difficulty attribute and rule keys
    const DIFFICULTY_ATTR: &str = "ME3gameDifficulty";
    const DIFFICULTY_RULE: &str = "ME3_gameDifficultyRule";

    /// Value for rules that have been abstained from matching
    /// when a rule is abstained it is ignored
    const ABSTAIN: &str = "abstain";

    /// Creates a new rule set from the provided list
    /// of rule key values
    ///
    /// `rules` The rules to create from
    pub fn new(rules: Vec<(String, String)>) -> Self {
        let mut map_rule: Option<String> = None;
        let mut enemy_rule: Option<String> = None;
        let mut difficulty_rule: Option<String> = None;

        for (rule, value) in rules {
            if value == Self::ABSTAIN {
                continue;
            }
            match &rule as &str {
                Self::MAP_RULE => map_rule = Some(value),
                Self::ENEMY_RULE => enemy_rule = Some(value),
                Self::DIFFICULTY_RULE => difficulty_rule = Some(value),
                _ => {}
            }
        }
        Self {
            map_rule,
            enemy_rule,
            difficulty_rule,
        }
    }

    /// Checks if the rules provided in this rule set match the values in
    /// the attributes map.
    ///
    /// `attributes` The attributes map to check for matches
    pub fn matches(&self, attributes: &AttrMap) -> bool {
        // Non public matches are unable to be matched
        if let Some(privacy) = attributes.get(Self::PRIVACY_ATTR) {
            if privacy != "PUBLIC" {
                return false;
            }
        }

        fn compare_rule(rule: Option<&String>, value: Option<&String>) -> bool {
            rule.zip(value)
                .map(|(a, b)| a.eq(b))
                // Missing rules / attributes count as match and continue
                .unwrap_or(true)
        }

        if !compare_rule(self.map_rule.as_ref(), attributes.get(Self::MAP_ATTR)) {
            return false;
        }

        if !compare_rule(self.enemy_rule.as_ref(), attributes.get(Self::ENEMY_ATTR)) {
            return false;
        }

        if !compare_rule(
            self.difficulty_rule.as_ref(),
            attributes.get(Self::DIFFICULTY_ATTR),
        ) {
            return false;
        }

        true
    }
}
