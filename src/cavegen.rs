use std::{collections::HashMap, error::Error, path::PathBuf};
use tokio::{fs::File, process::Command};

pub async fn run_cavegen(
    args: &HashMap<&'static str, String>,
) -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
    let cave = args
        .get("cave")
        .ok_or_else(|| "No valid cave/sublevel specifier was provided.")?;
    let seed = args.get("seed").ok_or_else(|| "No valid seed specified.")?;

    let mut extra_args = vec![];
    if args.get("251").is_some() {
        extra_args.push("-251");
    }
    else if args.get("new_year").is_some() {
        extra_args.push("-newYear")
    }
    if args.get("draw_score").is_some() {
        extra_args.push("-drawAllScores");
    }
    if let Some(region) = args.get("region") {
        extra_args.push("-region");
        extra_args.push(region);
    }

    invoke_cavegen_jar(&format!(
        "cave {} -seed {} -drawNoGateLife -quickglance {}",
        &cave,
        &seed,
        extra_args.join(" ")
    ))
    .await?;

    let cave_output_folder = {
        if cave == "colossal" {
            "./CaveGen/output/colossal-1".to_string()
        } else if args.get("251").is_some() {
            format!("./CaveGen/output251/{}", cave)
        } else if args.get("new_year").is_some() {
            format!("./CaveGen/outputNewYear/{}", cave)
        } else {
            format!("./CaveGen/output/{}", cave)
        }
    };

    let output_file: PathBuf = format!("{}/{}.png", &cave_output_folder, &seed[2..].to_ascii_uppercase()).into();
    if let Err(_) = File::open(&output_file).await {
        Err("Cavegen failed! This is probably a bug :(".into())
    } else {
        Ok(output_file)
    }
}

pub async fn run_caveinfo(
    args: &HashMap<&'static str, String>,
) -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
    let cave = args
        .get("cave")
        .ok_or_else(|| "No valid cave/sublevel specifier was provided.")?;
    let mut extra_args = vec![];
    if args.get("251").is_some() {
        extra_args.push("-251");
    }
    else if args.get("new_year").is_some() {
        extra_args.push("-newYear")
    }
    if let Some(region) = args.get("region") {
        extra_args.push("-region");
        extra_args.push(region);
    }

    invoke_cavegen_jar(&format!(
        "cave {} -caveInfoReport -drawAllWayPoints -drawSpawnPoints {}",
        &cave,
        extra_args.join(" ")
    ))
    .await?;

    let output_file = {
        if args.get("251").is_some() {
            format!("./CaveGen/output251/!caveinfo/{}.png", cave)
        } else if args.get("new_year").is_some() {
            format!("./CaveGen/outputNewYear/!caveinfo/{}.png", cave)
        } else {
            format!("./CaveGen/output/!caveinfo/{}.png", cave)
        }
    }
    .into();

    if let Err(_) = File::open(&output_file).await {
        Err("Cavegen failed! This is probably a bug :(".into())
    } else {
        Ok(output_file)
    }
}

async fn invoke_cavegen_jar(args: &str) -> std::io::Result<String> {
    let args = args.trim();
    let output = Command::new("java")
        .current_dir("./CaveGen")
        .arg("-jar")
        .arg("CaveGen.jar")
        .args(args.split(' '))
        .output()
        .await?;
    let stdout = std::str::from_utf8(output.stdout.as_slice())
        .expect("Java output was invalid UTF-8 when running Cavegen");
    println!("{}", stdout.to_string());
    Ok(stdout.to_string())
}

pub async fn clean_output_dir() {
    Command::new("rm")
        .arg("-rf")
        .arg("./CaveGen/output")
        .arg("./CaveGen/output251")
        .output()
        .await
        .expect("Failed to delete Cavegen output dir");
}
