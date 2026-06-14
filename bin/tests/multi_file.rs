use std::process::Command;

fn subject_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_statix"))
}

#[test]
fn statix_fix_on_multiple_nix_files() {
    let dir = tempfile::tempdir().unwrap();
    let file1 = dir.path().join("a.nix");
    let file2 = dir.path().join("b.nix");
    let file3 = dir.path().join("c.nix");

    // bool_simplification
    std::fs::write(&file1, "!(a == b)\n").unwrap();

    // manual_inherit
    std::fs::write(&file2, "let a = 2; in { a = a; }\n").unwrap();

    // eta_reduction
    std::fs::write(
        &file3,
        "let double = x: x * 2; in map (x: double x) [1 2 3]\n",
    )
    .unwrap();

    let status = subject_command()
        .arg("fix")
        .arg(&file1)
        .arg(&file2)
        .arg(&file3)
        .status()
        .unwrap();

    assert!(status.success());

    assert_eq!(std::fs::read_to_string(&file1).unwrap(), "a != b\n");
    assert_eq!(
        std::fs::read_to_string(&file2).unwrap(),
        "let a = 2; in { inherit a; }\n"
    );
    assert_eq!(
        std::fs::read_to_string(&file3).unwrap(),
        "let double = x: x * 2; in map double [1 2 3]\n"
    );
}

#[test]
fn statix_check_on_mulitple_nix_files() {
    let dir = tempfile::tempdir().unwrap();
    let file1 = dir.path().join("a.nix");
    let file2 = dir.path().join("b.nix");

    // bool_simplification
    std::fs::write(&file1, "!(a == b)\n").unwrap();

    // manual_inherit
    std::fs::write(&file2, "let a = 2; in { a = a; }\n").unwrap();

    let status = subject_command()
        .arg("check")
        .arg(&file1)
        .arg(&file2)
        .status()
        .unwrap();

    assert!(!status.success());
}
