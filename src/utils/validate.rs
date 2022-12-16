use regex::Regex;

/// Validates an email checking it against the email regex
pub fn is_email(email: &str) -> bool {
    let regex = Regex::new(
        r#"^([a-z0-9_+]([a-z0-9_+.]*[a-z0-9_+])?)@([a-z0-9]+([\-.][a-z0-9]+)*\.[a-z]{2,6})"#,
    )
    .unwrap();
    regex.is_match(email)
}
