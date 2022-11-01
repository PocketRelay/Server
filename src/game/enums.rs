use blaze_pk::TdfMap;

pub trait MatchRule: PartialEq {
    fn attr() -> &'static str;
    fn rule() -> &'static str;

    fn from_key(value: &str) -> Self;

    fn is_ignored(&self) -> bool;
}

macro_rules! match_rule {
    (

        NAME: $name:ident;
        ATTR: $attr:literal;
        RULE: $rule:literal;

        VALUES:
            $($field:ident($value:literal)),* $(,)?

    ) => {

        #[derive(Debug, Eq)]
        pub enum $name {
            $($field,)*
            Abstain,
            Other(String),
        }

        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                match self {
                    $(Self::$field => matches!(other, &Self::$field),)*
                    Self::Abstain => matches!(other, &Self::Abstain),
                    Self::Other(value) => match other {
                        Self::Other(value2) => value.eq(value2),
                        _ => false,
                    },
                }
            }
        }

        impl MatchRule for $name {

            fn attr() -> &'static str { $attr }
            fn rule() -> &'static str { $rule }

            fn from_key(value: &str) -> Self {

                match value {
                    $($value => Self::$field,)*
                    "abstain" => Self::Abstain,
                    value => Self::Other(value.to_string()),
                }

            }

            fn is_ignored(&self) -> bool {
                match self {
                    Self::Abstain => true,
                    Self::Other(_) => true,
                    _ => false,
                }
            }
        }

    };
}

match_rule! {
    NAME: EnemyType;
    ATTR: "ME3gameEnemyType";
    RULE: "ME3_gameEnemyTypeRule";
    VALUES:
        Random("random"),
        Cerberus("enemy1"),
        Geth("enemy2"),
        Reaper("enemy3"),
        Collector("enemy4"),
}

match_rule! {
    NAME: Difficulty;
    ATTR: "ME3gameDifficulty";
    RULE: "ME3_gameDifficultyRule";
    VALUES:
        Bronze("difficulty0"),
        Silver("difficulty1"),
        Gold("difficulty2"),
        Platinum("difficulty3"),
}

match_rule! {
    NAME: GameMap;
    ATTR: "ME3map";
    RULE: "ME3_gameMapMatchRule";
    VALUES:
        Unknown("map0"),
        Random("random"),
        FirebaseDagger("map2"),
        FirebaseGhost("map3"),
        FirebaseGiant("map4"),
}
