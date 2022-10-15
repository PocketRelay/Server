//! Logic for the custom simple dmap file format
use blaze_pk::TdfMap;

pub fn load_dmap(contents: &[u8]) -> TdfMap<String, String> {
    let contents = String::from_utf8_lossy(contents);
    let mut lines = contents.lines();
    let mut map = TdfMap::<String, String>::new();
    for line in lines {
        let (key, value) = match line.split_once("=") {
            Some(v) => v,
            _ => continue
        };
        map.insert(key, value)
    }
    map
}
