use super::AttrMap;

#[derive(Debug)]
pub struct Rule {
    // The rule key
    key: &'static str,
    // Game attribute represented by the rule
    attr: &'static str,
}

impl Rule {
    const fn new(key: &'static str, attr: &'static str) -> Self {
        Self { key, attr }
    }
}

/// Known rules and the attribute they operate over
pub static RULES: &[Rule] = &[
    // Map type
    Rule::new("ME3_gameMapMatchRule", "ME3map"),
    // Enemy type
    Rule::new("ME3_gameEnemyTypeRule", "ME3gameEnemyType"),
    // Difficulty type
    Rule::new("ME3_gameDifficultyRule", "ME3gameDifficulty"),
];

/// Rules for DLC that are present
pub static DLC_RULES: &[Rule] = &[
    // DLC Rules
    Rule::new("ME3_rule_dlc2300", "ME3_dlc2300"),
    Rule::new("ME3_rule_dlc2500", "ME3_dlc2500"),
    Rule::new("ME3_rule_dlc2700", "ME3_dlc2700"),
    Rule::new("ME3_rule_dlc3050", "ME3_dlc3050"),
    Rule::new("ME3_rule_dlc3225", "ME3_dlc3225"),
];

/// Attribute determining the game privacy for public
/// match checking
const PRIVACY_ATTR: &str = "ME3privacy";

/// Value for rules that have been abstained from matching
/// when a rule is abstained it is ignored
const ABSTAIN: &str = "abstain";

/// Defines a rule to be matched and the value to match
#[derive(Debug)]
pub struct MatchRule {
    /// Rule being matched for
    rule: &'static Rule,
    /// Value to match using
    value: String,
}

/// Set of rules to match
#[derive(Debug)]
pub struct RuleSet {
    /// The rules to match
    rules: Vec<MatchRule>,
}

impl RuleSet {
    /// Creates a new set of rule matches from the provided rule value pairs
    pub fn new(pairs: Vec<(String, String)>) -> Self {
        let mut rules = Vec::new();

        for (rule_key, value) in pairs {
            if value == ABSTAIN {
                continue;
            }

            let rule = RULES
                .iter()
                .chain(DLC_RULES.iter())
                .find(|rule| rule.key.eq(&rule_key));

            if let Some(rule) = rule {
                rules.push(MatchRule { rule, value })
            }
        }

        Self { rules }
    }

    /// Checks if the rules provided in this rule set match the values in
    /// the attributes map.
    pub fn matches(&self, attributes: &AttrMap) -> bool {
        // Non public matches are unable to be matched
        if let Some(privacy) = attributes.get(PRIVACY_ATTR) {
            if privacy != "PUBLIC" {
                return false;
            }
        }

        // Handle matching requested rules
        for rule in &self.rules {
            // Ensure the attribute is present and matching
            if !attributes
                .get(rule.rule.attr)
                .is_some_and(|value| value.eq(&rule.value))
            {
                return false;
            }
        }

        // Handle the game requiring a DLC rule but the client not specifying it
        for rule in DLC_RULES {
            let local_rule = self
                .rules
                .iter()
                .find(|match_rule| match_rule.rule.key == rule.key);
            let dlc_attribute = attributes.get(rule.attr);
            if dlc_attribute.is_some() && local_rule.is_none() {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod test {
    use crate::services::game::AttrMap;

    use super::RuleSet;

    /// Public match should succeed if the attributes meet the specified criteria
    #[test]
    fn test_public_match() {
        let attributes = [
            ("ME3_dlc2300", "required"),
            ("ME3_dlc2500", "required"),
            ("ME3_dlc2700", "required"),
            ("ME3_dlc3050", "required"),
            ("ME3_dlc3225", "required"),
            ("ME3gameDifficulty", "difficulty0"),
            ("ME3gameEnemyType", "enemy1"),
            ("ME3map", "map2"),
            ("ME3privacy", "PUBLIC"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<AttrMap>();

        let rules = [
            ("ME3_gameMapMatchRule", "abstain"),
            ("ME3_gameEnemyTypeRule", "abstain"),
            ("ME3_gameDifficultyRule", "abstain"),
            ("ME3_rule_dlc2500", "required"),
            ("ME3_rule_dlc2300", "required"),
            ("ME3_rule_dlc2700", "required"),
            ("ME3_rule_dlc3050", "required"),
            ("ME3_rule_dlc3225", "required"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<Vec<(String, String)>>();

        let rule_set = RuleSet::new(rules);

        let matches = rule_set.matches(&attributes);

        assert!(matches, "Rule set didn't match the provided attributes");
    }

    /// When attributes aren't abstain they should match exactly
    #[test]
    fn test_specific_attributes() {
        let attributes = [
            ("ME3_dlc2300", "required"),
            ("ME3_dlc2500", "required"),
            ("ME3_dlc2700", "required"),
            ("ME3_dlc3050", "required"),
            ("ME3_dlc3225", "required"),
            ("ME3gameDifficulty", "difficulty0"),
            ("ME3gameEnemyType", "enemy1"),
            ("ME3map", "map2"),
            ("ME3privacy", "PUBLIC"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<AttrMap>();

        let rules = [
            ("ME3_gameMapMatchRule", "map2"),
            ("ME3_gameEnemyTypeRule", "enemy1"),
            ("ME3_gameDifficultyRule", "difficulty0"),
            ("ME3_rule_dlc2500", "required"),
            ("ME3_rule_dlc2300", "required"),
            ("ME3_rule_dlc2700", "required"),
            ("ME3_rule_dlc3050", "required"),
            ("ME3_rule_dlc3225", "required"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<Vec<(String, String)>>();

        let rule_set = RuleSet::new(rules);

        let matches = rule_set.matches(&attributes);

        assert!(matches, "Rule set didn't match the provided attributes");
    }

    /// Private match should always fail a matchmaking rule set regardless
    /// of the other attributes
    #[test]
    fn test_private_match() {
        let attributes = [
            ("ME3_dlc2300", "required"),
            ("ME3_dlc2500", "required"),
            ("ME3_dlc2700", "required"),
            ("ME3_dlc3050", "required"),
            ("ME3_dlc3225", "required"),
            ("ME3gameDifficulty", "difficulty0"),
            ("ME3gameEnemyType", "enemy1"),
            ("ME3map", "map2"),
            ("ME3privacy", "PRIVATE"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<AttrMap>();

        let rules = [
            ("ME3_gameMapMatchRule", "abstain"),
            ("ME3_gameEnemyTypeRule", "abstain"),
            ("ME3_gameDifficultyRule", "abstain"),
            ("ME3_rule_dlc2500", "required"),
            ("ME3_rule_dlc2300", "required"),
            ("ME3_rule_dlc2700", "required"),
            ("ME3_rule_dlc3050", "required"),
            ("ME3_rule_dlc3225", "required"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<Vec<(String, String)>>();

        let rule_set = RuleSet::new(rules);

        let matches = rule_set.matches(&attributes);

        assert!(!matches, "Rule set matched a private match");
    }

    /// If the player has a DLC requirement that the host doesn't have
    /// the matching should fail
    #[test]
    fn test_dlc_mismatch() {
        let attributes = [
            ("ME3_dlc2300", "required"),
            ("ME3_dlc2500", "required"),
            ("ME3_dlc2700", "required"),
            ("ME3_dlc3050", "required"),
            ("ME3_dlc3225", "required"),
            ("ME3gameDifficulty", "difficulty0"),
            ("ME3gameEnemyType", "enemy1"),
            ("ME3map", "map2"),
            ("ME3privacy", "PUBLIC"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<AttrMap>();

        let rules = [
            ("ME3_gameMapMatchRule", "abstain"),
            ("ME3_gameEnemyTypeRule", "abstain"),
            ("ME3_gameDifficultyRule", "abstain"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<Vec<(String, String)>>();

        let rule_set = RuleSet::new(rules);

        let matches = rule_set.matches(&attributes);

        assert!(!matches, "Matched host with missing DLC");
    }

    /// If the host has required DLC but the player is missing it
    /// the matching should fail
    #[test]
    fn test_player_dlc_mismatch() {
        let attributes = [
            ("ME3gameDifficulty", "difficulty0"),
            ("ME3gameEnemyType", "enemy1"),
            ("ME3map", "map2"),
            ("ME3privacy", "PRIVATE"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<AttrMap>();

        let rules = [
            ("ME3_gameMapMatchRule", "abstain"),
            ("ME3_gameEnemyTypeRule", "abstain"),
            ("ME3_gameDifficultyRule", "abstain"),
            ("ME3_rule_dlc2500", "required"),
            ("ME3_rule_dlc2300", "required"),
            ("ME3_rule_dlc2700", "required"),
            ("ME3_rule_dlc3050", "required"),
            ("ME3_rule_dlc3225", "required"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<Vec<(String, String)>>();

        let rule_set = RuleSet::new(rules);

        let matches = rule_set.matches(&attributes);

        assert!(!matches, "Matched player with missing DLC");
    }
}
