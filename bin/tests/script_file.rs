use std::{fs, path::Path, process::Command};

fn statix(dir: &Path, args: &[&str]) -> String {
    let home = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_statix"))
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path())
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8(strip_ansi_escapes::strip(output.stdout)).unwrap()
}

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir(root.join("scripts")).unwrap();
    fs::write(
        root.join("scripts/deploy.sh"),
        "#!/usr/bin/env bash\ncd $TARGET\nrm -rf $TARGET/*\n",
    )
    .unwrap();
    fs::write(
        root.join("scripts/report.py"),
        "import os\nprint('report')\n",
    )
    .unwrap();
    fs::write(root.join("scripts/hook.sh"), "echo \"$out\"\n").unwrap();
    fs::write(root.join("scripts/clean.sh"), "echo \"clean\"\n").unwrap();
    fs::write(
        root.join("devenv.nix"),
        r#"{ pkgs, ... }:
{
  scripts.deploy.exec = ./scripts/deploy.sh;
  scripts.report.exec = "python scripts/report.py";
  scripts.again.exec = ''bash "${./scripts/deploy.sh}"'';
  scripts.clean.exec = ./scripts/clean.sh;
  packages = [ (pkgs.makeSetupHook { name = "x"; } ./scripts/hook.sh) ];
}
"#,
    )
    .unwrap();
    dir
}

#[test]
fn reports_findings_of_referenced_scripts() {
    let dir = project();
    let out = statix(dir.path(), &["check", "-o", "errfmt", "devenv.nix"]);
    insta::assert_snapshot!(out);
}

#[test]
fn fix_rewrites_referenced_scripts() {
    let dir = project();
    let root = dir.path();
    statix(root, &["fix", "devenv.nix"]);
    insta::assert_snapshot!(
        "deploy.sh",
        fs::read_to_string(root.join("scripts/deploy.sh")).unwrap()
    );
    insta::assert_snapshot!(
        "report.py",
        fs::read_to_string(root.join("scripts/report.py")).unwrap()
    );
    // the Nix file itself is untouched
    assert!(
        fs::read_to_string(root.join("devenv.nix"))
            .unwrap()
            .contains("./scripts/deploy.sh")
    );
}

#[test]
fn dry_run_shows_diff_without_writing() {
    let dir = project();
    let root = dir.path();
    let out = statix(root, &["fix", "--dry-run", "devenv.nix"]);
    assert!(out.contains("scripts/deploy.sh [fixed]"), "{out}");
    assert!(
        fs::read_to_string(root.join("scripts/deploy.sh"))
            .unwrap()
            .contains("cd $TARGET\n")
    );
}

#[test]
fn agent_format_shows_the_script_file() {
    let dir = project();
    let out = statix(dir.path(), &["check", "-o", "agent", "devenv.nix"]);
    insta::assert_snapshot!(out.replace(&dir.path().display().to_string(), "<dir>"));
}

#[test]
fn a_changed_script_checks_the_nix_files_referring_to_it() {
    let dir = project();
    let root = dir.path();
    fs::write(root.join("other.nix"), "{ enterShell = \"cd $HOME\"; }\n").unwrap();
    // only the script changed, as a git hook would pass it
    let out = statix(root, &["check", "-o", "errfmt", "scripts/deploy.sh"]);
    assert!(
        out.contains("devenv.nix>3:25:W:30:[script_file] ./scripts/deploy.sh:2:1: SC2164"),
        "{out}"
    );
    assert!(!out.contains("other.nix"), "{out}");
}

#[test]
fn several_targets() {
    let dir = project();
    let root = dir.path();
    fs::write(root.join("other.nix"), "{ enterShell = \"cd $HOME\"; }\n").unwrap();
    fs::write(root.join("clean.nix"), "{ enterShell = \"echo hi\"; }\n").unwrap();
    let out = statix(root, &["check", "-o", "errfmt", "other.nix", "clean.nix"]);
    assert!(out.contains("other.nix>1:"), "{out}");
    assert!(!out.contains("devenv.nix"), "{out}");
}

#[test]
fn fix_with_a_changed_script_fixes_it() {
    let dir = project();
    let root = dir.path();
    statix(root, &["fix", "scripts/deploy.sh"]);
    let fixed = fs::read_to_string(root.join("scripts/deploy.sh")).unwrap();
    assert!(fixed.contains("cd \"$TARGET\" || exit"), "{fixed}");
}
