use tokio::process::Command;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref SUBLEVEL_ID_RE: Regex = Regex::new(r"([[:alpha:]]{2,3})[_-]?(\d+)").unwrap();
    static ref CAVES: [&'static str; 14] = [
        "EC", "SCx", "FC", "HoB", "WFG", "SH", "BK", "CoS", "GK", "SC", "SR", "CoC", "DD", "HoH"
    ];
}

pub async fn invoke_cavegen(args: &str) -> std::io::Result<String> {
    let output = Command::new("java")
        .current_dir("./CaveGen")
        .arg("-jar")
        .arg("CaveGen.jar")
        .args(args.split(' '))
        .output()
        .await?;
    let stdout = std::str::from_utf8(output.stdout.as_slice())
        .expect("Java output was invalid UTF-8 when running Cavegen");
    Ok(stdout.to_string())
}

pub async fn clean_output_dir() {
    Command::new("rm")
        .arg("-rf")
        .arg("./CaveGen/output")
        .output()
        .await
        .expect("Failed to delete Cavegen output dir");
}

pub fn normalize_sublevel_id(raw: &str) -> Option<String> {
    let captures = SUBLEVEL_ID_RE.captures(raw)?;
    let cave_name = captures.get(1)?.as_str();
    let sublevel = captures.get(2)?.as_str();

    let cave_name_normalized = CAVES.iter()
        .find(|cave| cave_name.eq_ignore_ascii_case(cave))?;
    Some(format!("{}-{}", cave_name_normalized, sublevel))
}
