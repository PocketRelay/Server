//! Utility for loading dmap files
use blaze_pk::types::TdfMap;

/// Loads the dmap format from the provided string.
///
/// # Format
/// Items are split using lines and key values are split using the
/// first equals sign any other equals signs on the line are included
/// in the value portion
///
/// ```
/// KEY=VALUE
/// ABC=ADWADAWDaWDAWd==awdaw
/// ```
///
/// `contents` The dmap format string
pub fn load_dmap(contents: &str) -> TdfMap<String, String> {
    let mut map = TdfMap::<String, String>::new();
    for line in contents.lines() {
        let (key, value) = match line.split_once("=") {
            Some(v) => v,
            _ => continue,
        };
        map.insert(key, value)
    }
    map
}

#[cfg(test)]
mod test {
    use super::load_dmap;

    /// Tests the DMAP format by decoding an example map
    #[test]
    fn test_load_dmap() {
        let map = "TEST=ABC\nABC=VALUE\nVALUE=TEST_2131238udadwa==0ccdwadawd";
        let value = load_dmap(map);

        assert_eq!(value.get("TEST"), Some(&"ABC".to_string()));
        assert_eq!(value.get("ABC"), Some(&"VALUE".to_string()));
        assert_eq!(value.get("OTHER"), None);
        assert_eq!(value.get("1"), None);
        assert_eq!(
            value.get("VALUE"),
            Some(&"TEST_2131238udadwa==0ccdwadawd".to_string())
        );
    }
}
