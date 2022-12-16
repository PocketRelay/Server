use rand_core::{OsRng, RngCore};

pub fn generate_random_string(len: usize) -> String {
    const RANGE: u32 = 26 + 26 + 10;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                abcdefghijklmnopqrstuvwxyz\
                0123456789";

    let mut rand = OsRng;
    let mut output = String::with_capacity(len);

    // Loop until the string length is finished
    for _ in 0..len {
        // Loop until a valid random is found
        loop {
            let var = rand.next_u32() >> (32 - 6);
            if var < RANGE {
                output.push(char::from(CHARSET[var as usize]));
                break;
            }
        }
    }

    output
}

#[cfg(test)]
mod test {
    use super::generate_random_string;

    #[test]
    fn test_random() {
        let value = generate_random_string(128);
        println!("Generated: {value:?}");
        assert!(value.len() == 128)
    }
}
