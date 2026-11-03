// Macro to generate the test runner functions
#[doc(hidden)]
#[macro_export]
macro_rules! __database_test_case {
    // Macro for generating the test boilerplate to run a sqlite test
    (sqlite, $test_name:ident) => {
        #[tokio::test]
        async fn $test_name() {
            let db = $crate::common::database::sqlite_database().await;
            super::$test_name(&db).await;
        }
    };
    // Macro for generating the test boilerplate to run a postgres test
    (postgres, $test_name:ident) => {
        #[tokio::test]
        async fn $test_name() {
            let (db, _postgres) = $crate::common::database::postgres_database().await;
            super::$test_name(&db).await;
        }
    };
}

// Macro for generating the test boilerplate to run a collection of
// test functions against both sqlite and postgres databases
#[macro_export]
macro_rules! database_tests {
    ($($test_name:ident),* $(,)?) => {
        mod __sqlite {
            $($crate::__database_test_case!(sqlite, $test_name);)*
        }

        mod __postgres {
            $($crate::__database_test_case!(postgres, $test_name);)*
        }
    };
}
