use std::{
    fs,
    io::{self, Error, ErrorKind},
    path::{Path, PathBuf},
};

use crate::dirs;

use ignore::{
    Error as IgnoreError, Match,
    gitignore::{Gitignore, GitignoreBuilder},
};

#[derive(Debug)]
pub struct Walker {
    dirs: Vec<(PathBuf, Vec<PathBuf>, Gitignore)>,
    files: Vec<PathBuf>,
    extra_ignores: Vec<String>,
    unrestricted: bool,
}

impl Walker {
    pub fn new<P: AsRef<Path>>(
        target: P,
        ignore: Gitignore,
        extra_ignores: Vec<String>,
        unrestricted: bool,
    ) -> io::Result<Self> {
        let target = target.as_ref().to_path_buf();
        if !target.exists() {
            Err(Error::new(
                ErrorKind::NotFound,
                format!("file not found: {}", target.display()),
            ))
        } else if target.is_dir() {
            let root_gitignore = target.join(".gitignore");
            let gitignore_files = if !unrestricted && root_gitignore.exists() {
                vec![root_gitignore]
            } else {
                vec![]
            };
            Ok(Self {
                dirs: vec![(target, gitignore_files, ignore)],
                files: vec![],
                extra_ignores,
                unrestricted,
            })
        } else {
            Ok(Self {
                dirs: vec![],
                files: vec![target],
                extra_ignores,
                unrestricted,
            })
        }
    }

    fn build_ignore_for(
        &self,
        base: &Path,
        gitignore_files: &[PathBuf],
    ) -> Result<Gitignore, IgnoreError> {
        let mut builder = GitignoreBuilder::new(base);

        if !self.unrestricted {
            for gitignore in gitignore_files {
                builder.add(gitignore);
            }

            builder.add_line(None, ".git")?;
        }
        for ignore in &self.extra_ignores {
            builder.add_line(None, ignore.as_str())?;
        }
        builder.build()
    }
}

impl Iterator for Walker {
    type Item = PathBuf;
    fn next(&mut self) -> Option<Self::Item> {
        self.files.pop().or_else(|| {
            while let Some((dir, mut gitignore_files, mut ignore)) = self.dirs.pop() {
                if !dir.is_dir() {
                    continue;
                }
                let nested = dir.join(".gitignore");

                if !self.unrestricted && nested.exists() && !gitignore_files.contains(&nested) {
                    gitignore_files.push(nested);

                    ignore = match self.build_ignore_for(&dir, &gitignore_files) {
                        Ok(ignore) => ignore,
                        Err(_) => continue,
                    };
                }

                if !matches!(
                    ignore.matched(&dir, true),
                    Match::None | Match::Whitelist(_)
                ) {
                    continue;
                }

                let mut found = false;

                for entry in fs::read_dir(&dir).ok()? {
                    let entry = entry.ok()?;
                    let path = entry.path();

                    if path.is_dir() {
                        self.dirs
                            .push((path, gitignore_files.clone(), ignore.clone()));
                    } else if path.is_file()
                        && matches!(
                            ignore.matched(&path, false),
                            Match::None | Match::Whitelist(_)
                        )
                    {
                        found = true;
                        self.files.push(path);
                    }
                }

                if found {
                    break;
                }
            }
            self.files.pop()
        })
    }
}

pub fn build_ignore_set<P: AsRef<Path>>(
    ignore: &[String],
    target: P,
    unrestricted: bool,
) -> Result<Gitignore, IgnoreError> {
    let gitignore_path = target.as_ref().join(".gitignore");

    // Looks like GitignoreBuilder::new does not source globs
    // within gitignore_path by default, we have to enforce that
    // using GitignoreBuilder::add. Probably a bug in the ignore
    // crate?
    let mut gitignore = GitignoreBuilder::new(&gitignore_path);

    // if we are to "restrict" aka "respect" .gitignore, then
    // add globs from gitignore path as well
    if !unrestricted {
        gitignore.add(&gitignore_path);

        // ignore .git by default, nobody cares about .git, i'm sure
        gitignore.add_line(None, ".git")?;
    }

    for i in ignore {
        gitignore.add_line(None, i.as_str())?;
    }

    gitignore.build()
}

pub fn walk_nix_files<P: AsRef<Path>>(
    ignore: Gitignore,
    target: P,
    extra_ignores: &[String],
    unrestricted: bool,
) -> Result<impl Iterator<Item = PathBuf>, io::Error> {
    let walker = dirs::Walker::new(target, ignore, extra_ignores.to_vec(), unrestricted)?;
    Ok(walker
        .filter(|path: &PathBuf| matches!(path.extension(), Some(extension) if extension == "nix")))
}
