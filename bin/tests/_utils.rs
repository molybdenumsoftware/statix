use std::{io::Write, process::Command, process::Stdio};

use tempfile::NamedTempFile;

#[allow(dead_code)]
pub fn test_cli(expression: &str, args: &[&str]) -> anyhow::Result<String> {
    let mut fixture = NamedTempFile::with_suffix(".nix")?;
    fixture.write_all(expression.as_bytes())?;

    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .args(args)
        .arg(fixture.path())
        .output()?;

    let stdout = strip_ansi_escapes::strip(output.stdout)?;
    let stdout = String::from_utf8(stdout)?;
    let stdout = stdout.replace(fixture.path().to_str().unwrap(), "<temp_file_path>");

    Ok(stdout)
}

#[allow(dead_code)]
pub fn test_cli_stdin(input: &str, args: &[&str]) -> anyhow::Result<String> {
    let mut child = Command::new("cargo")
        .arg("run")
        .arg("--")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(input.as_bytes())?;

    let output = child.wait_with_output()?;
    let stdout = strip_ansi_escapes::strip(output.stdout)?;

    Ok(String::from_utf8(stdout)?)
}
