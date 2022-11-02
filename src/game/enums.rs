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
        // Standard Maps
        FirebaseDagger("map2"),
        FirebaseGhost("map3"),
        FirebaseGiant("map4"),
        FirebaseReactor("map5"),
        FirebaseGlacier("map7"),
        FirebaseWhite("map8"),
        // Resurgence Pack Maps
        FirebaseCondor("map9"),
        FirebaseHydra("map10"),
        // Rebellion Pack Maps
        FirebaseJade("map11"),
        FirebaseGoddess("map13"),
        // Earth Maps
        FirebaseRio("map14"),
        FirebaseVancouver("map15"),
        FirebaseLondon("map16"),
        // Retaliation Hazard Maps
        FirebaseGlacierHazard("map17"),
        FirebaseDaggerHazard("map18"),
        FirebaseReactorHazard("map19"),
        FirebaseGhostHazard("map20"),
        FirebaseGiantHazard("map21"),
        FirebaseWhiteHazard("map22")

        // Other Unknowns: map1, map6, map12, map23, map24
        // map25, map26, map27, map28, map29

}
