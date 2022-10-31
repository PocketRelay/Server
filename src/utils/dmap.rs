//! Logic for the custom simple dmap file format
use blaze_pk::TdfMap;

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
