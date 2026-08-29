use std::path::{Path, PathBuf};

use ignore::{Error as IgnoreError, Match, WalkBuilder, gitignore::GitignoreBuilder};

pub fn check_path_exists<P: AsRef<Path>>(target: P) -> std::io::Result<()> {
    let target = target.as_ref();
    if target.exists() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("file not found: {}", target.display()),
        ))
    }
}

pub fn walk_nix_files<P: AsRef<Path>>(
    target: P,
    extra_ignores: &[String],
    unrestricted: bool,
) -> Result<impl Iterator<Item = PathBuf>, IgnoreError> {
    let target = target.as_ref();

    let mut builder = WalkBuilder::new(target);

    // read as "do not ignore hidden files"
    builder.hidden(false);

    if unrestricted {
        builder.standard_filters(false);
    } else {
        builder.require_git(false);
    }

    let mut gitignore = GitignoreBuilder::new(target);

    if !unrestricted {
        // ignore .git by default, nobody cares about .git, i'm sure
        gitignore.add_line(None, ".git")?;
    }

    for ignore_rule in extra_ignores {
        gitignore.add_line(None, ignore_rule)?;
    }
    let custom_ignore = gitignore.build()?;

    builder.filter_entry(move |entry| {
        if entry.depth() == 0 {
            return true;
        }

        let is_dir = entry.file_type().is_some_and(|ft| ft.is_dir());

        !matches!(
            custom_ignore.matched(entry.path(), is_dir),
            Match::Ignore(_)
        )
    });

    Ok(builder.build().filter_map(|entry| {
        let entry = entry.ok()?;
        let file_type = entry.file_type()?;

        if !file_type.is_file() {
            return None;
        }

        let path = entry.into_path();
        if path.extension()? != "nix" {
            return None;
        }

        Some(path)
    }))
}
