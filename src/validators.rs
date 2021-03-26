use lazy_static::lazy_static;
use maplit::hashmap;
use regex::Regex;
use std::collections::HashMap;

lazy_static! {
    static ref CAVES: HashMap<&'static str, u16> = hashmap! {
        "EC" => 2,
        "SCx" => 9,
        "FC" => 8,
        "HoB" => 5,
        "WFG" => 5,
        "SH" => 7,
        "BK" => 7,
        "CoS" => 5,
        "GK" => 6,
        "SR" => 7,
        "SC" => 5,
        "CoC" => 10,
        "HoH" => 15,
        "DD" => 14
    };
    static ref HEX: Regex = Regex::new(r"0x[0-9A-F]{8}").unwrap();
}

pub fn sublevel_valid(sublevel: &str) -> bool {
    if let Some((cave, level)) = sublevel.split_once('-') {
        CAVES
            .get(cave)
            .and_then(|max_floors| Some(level.parse::<u16>().ok()? <= *max_floors))
            .unwrap_or(false)
    } else {
        false
    }
}

pub fn seed_valid(seed: &str) -> bool {
    HEX.is_match(seed) && seed.len() == 10
}
