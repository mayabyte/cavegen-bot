use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref HEX: Regex = Regex::new(r"0x[0-9A-F]{8}").unwrap();
}

pub fn seed_valid(seed: &str) -> bool {
    HEX.is_match(seed) && seed.len() == 10
}
