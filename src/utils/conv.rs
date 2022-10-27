use std::str::{FromStr, Split};

struct MEStringParser<'a> {
    values: Split<'a, &'static str>,
}

impl<'a> MEStringParser<'a> {
    pub fn new(value: &'a str) -> MEStringParser<'a> {
        MEStringParser {
            values: value.split(";")
        }
    }

    pub fn skip(&mut self, count: usize) -> Option<()> {
        for _ in 0..count {
            self.values.next()?;
        }
        Some(())
    }

    pub fn next_str(&mut self) -> Option<String> {
        let next = self.values.next()?;
        Some(next.to_string())
    }

    pub fn next<F: FromStr>(&mut self) -> Option<F> {
        let next = self.values.next()?;
        next.parse::<F>()
            .ok()
    }
}



#[cfg(test)]
mod test {
    use crate::utils::conv::MEStringParser;

    #[test]
    fn test_a() {
        let value = "AABB;123;DWADA";
        let mut parser = MEStringParser::new(value);
        assert_eq!(parser.next_str().unwrap(), "AABB");
        assert_eq!(parser.next::<u16>().unwrap(), 123);
        assert_eq!(parser.next_str().unwrap(), "DWADA");
    }
}