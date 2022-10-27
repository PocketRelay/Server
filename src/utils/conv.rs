use std::str::{FromStr, Split};

/// Structure for parsing ME3 format strings which are strings made up of sets
/// of data split by ; that each start with the 20;4;
///
/// # Example
/// ```20;4;Sentinel;20;0.0000;50```
pub struct MEStringParser<'a> {
    split: Split<'a, &'static str>,
}

impl<'a> MEStringParser<'a> {
    pub fn new(value: &'a str) -> Option<MEStringParser<'a>> {
        if !value.starts_with("20;4;") {
            return None
        }
        let split = value[5..].split(";");
        Some(MEStringParser {
            split
        })
    }

    pub fn skip(&mut self, count: usize) -> Option<()> {
        for _ in 0..count {
            self.split.next()?;
        }
        Some(())
    }

    pub fn next_str(&mut self) -> Option<String> {
        let next = self.split.next()?;
        Some(next.to_string())
    }

    pub fn next<F: FromStr>(&mut self) -> Option<F> {
        let next = self.split.next()?;
        next.parse::<F>()
            .ok()
    }
}



#[cfg(test)]
mod test {
    use crate::utils::conv::MEStringParser;

    #[test]
    fn test_a() {
        let value = "20;4;AABB;123;DWADA";
        let mut parser = MEStringParser::new(value)
            .unwrap();
        assert_eq!(parser.next_str().unwrap(), "AABB");
        assert_eq!(parser.next::<u16>().unwrap(), 123);
        assert_eq!(parser.next_str().unwrap(), "DWADA");
    }
}