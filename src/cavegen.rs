use tokio::process::Command;

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
