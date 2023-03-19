use argon2::password_hash::rand_core::{OsRng, RngCore};

/// Creates a random string of the provided length
///
/// `length` The length of the random string
pub fn random_string(length: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
    abcdefghijklmnopqrstuvwxyz\
    0123456789";
    const RANGE: usize = CHARSET.len();

    let mut rand = OsRng;
    let mut output = String::with_capacity(length);

    // Loop until the string length is finished
    for _ in 0..length {
        // Loop until a valid random is found
        loop {
            let var = (rand.next_u32() >> (32 - 6)) as usize;
            if var < RANGE {
                output.push(char::from(CHARSET[var]));
                break;
            }
        }
    }

    output
}
