use lazy_static::lazy_static;
use regex::Regex;
use serenity::framework::standard::Args;
use std::collections::HashMap;

lazy_static! {
    static ref HEX: Regex = Regex::new(r"0x[0-9A-Fa-f]{8}").unwrap();
    static ref SUBLEVEL_ID_RE: Regex = Regex::new(r"([[:alpha:]]{2,5})[_-]?(\d+)").unwrap();
    static ref CHALLENGE_MODE_ID_RE: Regex = Regex::new(r"[cC][hH]\d+[_-]\d+").unwrap();
    static ref CAVES: [&'static str; 41] = [
        "EC", "SCx", "FC", "HoB", "WFG", "SH", "BK", "CoS", "GK", "SC", "SR", "CoC", "DD", "HoH",
        "AT", "IM", "AD", "GD", "FT", "WF", "GdD", "AS", "SS", "CK", "PoW", "PoM", "EA", "DD",
        "PP", "BG", "SK", "CwNN", "SnD", "CH", "RH", "SA", "AA", "TC", "ER", "CG", "SD"
    ];
}

pub fn extract_standard_args(mut args: Args) -> HashMap<&'static str, String> {
    let mut arg_map = HashMap::new();
    for arg in args.iter::<String>() {
        let arg: String = arg.unwrap(); // All arguments can safely be parsed as strings.

        if match_seed(&arg) {
            arg_map.insert("seed", arg);
        } else if let Some(cave) = match_cave_specifier(&arg) {
            arg_map.insert("cave", cave);
        } else if arg == "+251" {
            arg_map.insert("251", "yes".to_string());
        } else if arg.eq_ignore_ascii_case("+newyear") || arg.eq_ignore_ascii_case("+new_year") {
            arg_map.insert("new_year", "yes".to_string());
        } else if arg.eq_ignore_ascii_case("+score") {
            arg_map.insert("draw_score", "yes".to_string());
        } else if arg.eq_ignore_ascii_case("+jp") || arg.eq_ignore_ascii_case("+jpn") {
            arg_map.insert("region", "jpn".to_string());
        }
        // There seems to be a bug in CaveGen itself that makes the PAL region option
        // behave the same as JP. For now this will be disabled.
        // else if arg.eq_ignore_ascii_case("+pal") {
        //     arg_map.insert("region", "pal".to_string());
        // }
        else if arg.eq_ignore_ascii_case("help") {
            arg_map.insert("help", "yes".to_string());
        }
    }
    arg_map
}

fn match_seed(seed: &str) -> bool {
    HEX.is_match(seed) && seed.len() == 10
}

/// Attempts to transform a valid but potentially non-strict specifier into a
/// strict one in Cavegen's expected format.
fn match_cave_specifier(raw: &str) -> Option<String> {
    match raw.to_ascii_lowercase().as_ref() {
        "colossal" => Some("colossal".to_string()),
        ch if CHALLENGE_MODE_ID_RE.is_match(ch) => Some(ch.to_ascii_uppercase()),
        _ => normalize_sublevel(raw),
    }
}

/// Transforms a non-strict sublevel specifier (e.g. scx1) into the format Cavegen
/// expects (SCx-1).
fn normalize_sublevel(raw: &str) -> Option<String> {
    let captures = SUBLEVEL_ID_RE.captures(raw)?;
    let cave_name = captures.get(1)?.as_str();
    let sublevel = captures.get(2)?.as_str();

    let cave_name_normalized = CAVES
        .iter()
        .find(|cave| cave_name.eq_ignore_ascii_case(cave))?;
    Some(format!("{}-{}", cave_name_normalized, sublevel))
}
