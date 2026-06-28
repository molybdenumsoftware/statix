use std::{collections::HashSet, process::Command};

const CODE_THAT_TRIGGERS_A_LINT: &str = "!(a == b)\n";

#[derive(Debug)]
struct Paths(HashSet<String>);

impl<const N: usize> PartialEq<[&str; N]> for Paths {
    fn eq(&self, other: &[&str; N]) -> bool {
        self.0.len() == N && other.iter().all(|&s| self.0.contains(s))
    }
}

struct Fixture {
    files: Vec<(String, String)>,
}

impl Fixture {
    fn with_files(files: &[(&str, &str)]) -> Self {
        Self {
            files: files
                .iter()
                .map(|&(path, content)| (path.to_owned(), content.to_owned()))
                .collect(),
        }
    }

    fn run_with_args(self, args: &[&str]) -> Result<Report, Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;

        for (relative_path, content) in &self.files {
            let path = dir.path().join(relative_path);
            std::fs::create_dir_all(path.parent().unwrap())?;
            std::fs::write(&path, content)?;
        }

        let output = Command::new(env!("CARGO_BIN_EXE_statix"))
            .current_dir(dir.path())
            .arg("check")
            .args(["-o", "errfmt"])
            .args(args)
            .output()?;

        let paths = std::str::from_utf8(&output.stdout)?
            .lines()
            .map(|line| {
                let (path, _) = line
                    .split_once('>')
                    .ok_or("errfmt line did not contain '>'")?;
                Ok(path.to_owned())
            })
            .collect::<Result<_, Box<dyn std::error::Error>>>()?;

        Ok(Report {
            paths: Paths(paths),
        })
    }
}

struct Report {
    paths: Paths,
}

mod single_file {
    use super::*;

    #[test]
    fn only_it_is_linted() {
        let report = Fixture::with_files(&[
            ("a.nix", CODE_THAT_TRIGGERS_A_LINT),
            ("b.nix", CODE_THAT_TRIGGERS_A_LINT),
        ])
        .run_with_args(&["a.nix"])
        .unwrap();

        assert_eq!(report.paths, ["a.nix"]);
    }
}

mod directory {
    use super::*;

    #[test]
    fn default_is_current_directory() {
        let report = Fixture::with_files(&[("a.nix", CODE_THAT_TRIGGERS_A_LINT)])
            .run_with_args(&[])
            .unwrap();

        assert_eq!(report.paths, ["./a.nix"]);
    }

    #[test]
    fn all_nix_files_are_linted() {
        let report = Fixture::with_files(&[
            ("dir/a.nix", CODE_THAT_TRIGGERS_A_LINT),
            ("dir/b.nix", CODE_THAT_TRIGGERS_A_LINT),
        ])
        .run_with_args(&["dir"])
        .unwrap();

        assert_eq!(report.paths, ["dir/a.nix", "dir/b.nix"]);
    }

    #[test]
    fn nix_files_in_subdirs_are_linted() {
        let report = Fixture::with_files(&[
            ("dir/top.nix", CODE_THAT_TRIGGERS_A_LINT),
            ("dir/sub/nested.nix", CODE_THAT_TRIGGERS_A_LINT),
        ])
        .run_with_args(&["dir"])
        .unwrap();

        assert_eq!(report.paths, ["dir/top.nix", "dir/sub/nested.nix"]);
    }

    #[test]
    fn non_nix_files_are_excluded() {
        let report = Fixture::with_files(&[
            ("dir/a.nix", CODE_THAT_TRIGGERS_A_LINT),
            ("dir/b.rs", CODE_THAT_TRIGGERS_A_LINT),
            ("dir/c.py", CODE_THAT_TRIGGERS_A_LINT),
        ])
        .run_with_args(&["dir"])
        .unwrap();

        assert_eq!(report.paths, ["dir/a.nix"]);
    }
}

mod gitignored_files {
    use super::*;

    #[test]
    fn are_excluded() {
        let report = Fixture::with_files(&[
            (".gitignore", "ignored.nix\n"),
            ("linted.nix", CODE_THAT_TRIGGERS_A_LINT),
            ("ignored.nix", CODE_THAT_TRIGGERS_A_LINT),
        ])
        .run_with_args(&[])
        .unwrap();

        assert_eq!(report.paths, ["./linted.nix"]);
    }

    #[test]
    fn in_subdirs_are_excluded() {
        let report = Fixture::with_files(&[
            (".gitignore", "generated/\n"),
            ("linted.nix", CODE_THAT_TRIGGERS_A_LINT),
            ("generated/inside.nix", CODE_THAT_TRIGGERS_A_LINT),
        ])
        .run_with_args(&[])
        .unwrap();

        assert_eq!(report.paths, ["./linted.nix"]);
    }
}

mod unrestricted {
    use super::*;

    #[test]
    fn bypasses_gitignore() {
        let report = Fixture::with_files(&[
            (".gitignore", "ignored.nix\n"),
            ("linted.nix", CODE_THAT_TRIGGERS_A_LINT),
            ("ignored.nix", CODE_THAT_TRIGGERS_A_LINT),
        ])
        .run_with_args(&["--unrestricted"])
        .unwrap();

        assert_eq!(report.paths, ["./linted.nix", "./ignored.nix"]);
    }
}

mod ignore_flag {
    use super::*;

    #[test]
    fn excludes_matching_file() {
        let report = Fixture::with_files(&[
            ("linted.nix", CODE_THAT_TRIGGERS_A_LINT),
            ("generated.nix", CODE_THAT_TRIGGERS_A_LINT),
        ])
        .run_with_args(&["--ignore", "generated.nix"])
        .unwrap();

        assert_eq!(report.paths, ["./linted.nix"]);
    }

    #[test]
    fn applies_even_when_unrestricted() {
        let report = Fixture::with_files(&[
            (".gitignore", "ignored.nix\n"),
            ("linted.nix", CODE_THAT_TRIGGERS_A_LINT),
            ("ignored.nix", CODE_THAT_TRIGGERS_A_LINT),
        ])
        .run_with_args(&["--unrestricted", "--ignore", "ignored.nix"])
        .unwrap();

        assert_eq!(report.paths, ["./linted.nix"]);
    }
}
