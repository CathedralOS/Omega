use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOptions {
    pub root_path: PathBuf,
    pub build_dir: Option<PathBuf>,
    pub target_name: Option<String>,
}

impl CompileOptions {
    pub fn build_dir(&self) -> PathBuf {
        self.build_dir.clone().unwrap_or_else(|| {
            self.root_path
                .parent()
                .map(|parent| parent.join("build"))
                .unwrap_or_else(|| PathBuf::from("build"))
        })
    }

    /// Physical identity of this compilation's staging directory, for
    /// collision checks between independently configured targets.
    ///
    /// Comparison happens before any directory is created, so the identity
    /// resolves `.` and `..` lexically, joins relative spellings onto the
    /// process working directory where staging resolves them, and
    /// canonicalizes the longest existing ancestor to catch symlink and
    /// filesystem-case aliases. A not-yet-created tail reattaches verbatim:
    /// spellings that alias only through a link inside a missing subtree still
    /// compare distinct, while every collision among existing prefixes is
    /// caught before two targets race writes into one directory.
    pub(crate) fn build_dir_identity(&self) -> PathBuf {
        staging_directory_identity(&self.build_dir())
    }

    /// Resolve and retain the artifact root before a caller replaces the
    /// source entrypoint with an immutable compilation-custody path.
    ///
    /// An explicit build directory is preserved byte-for-byte. Otherwise the
    /// default remains beside the entrypoint that the user supplied.
    pub fn retain_build_dir(&mut self) -> PathBuf {
        let build_dir = self.build_dir();
        self.build_dir = Some(build_dir.clone());
        build_dir
    }
}

fn staging_directory_identity(directory: &Path) -> PathBuf {
    // Resolve `.` and resolve `..` over ordinary components without touching
    // the filesystem. A `..` at the root is a physical no-op; a `..` with no
    // ordinary component to pop stays literal and keeps its meaning once the
    // path is absolutized below.
    let mut lexical = PathBuf::new();
    for component in directory.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match lexical.components().next_back() {
                Some(Component::Normal(_)) => {
                    lexical.pop();
                }
                Some(Component::RootDir) | Some(Component::Prefix(_)) => {}
                _ => lexical.push(component.as_os_str()),
            },
            _ => lexical.push(component.as_os_str()),
        }
    }
    let absolute = if lexical.is_absolute() {
        lexical
    } else {
        match std::env::current_dir() {
            Ok(cwd) => cwd.join(lexical),
            Err(_) => lexical,
        }
    };
    // Canonicalize the longest existing ancestor: symlinks, case spelling and
    // the working-directory join all resolve there. Every component below the
    // first missing one is an ordinary name reattached verbatim.
    let mut tail = Vec::new();
    let mut cursor = absolute.as_path();
    loop {
        match cursor.canonicalize() {
            Ok(canonical) => {
                return tail
                    .iter()
                    .rev()
                    .fold(canonical, |path, name| path.join(name));
            }
            Err(_) => match (cursor.file_name(), cursor.parent()) {
                (Some(name), Some(parent)) => {
                    tail.push(name.to_os_string());
                    cursor = parent;
                }
                _ => return absolute,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CompileOptions, staging_directory_identity};
    use std::path::{Path, PathBuf};

    #[test]
    fn retained_default_build_dir_survives_compilation_root_retargeting() {
        let mut options = CompileOptions {
            root_path: PathBuf::from("authored/project/main.omg"),
            build_dir: None,
            target_name: None,
        };

        let retained = options.retain_build_dir();
        options.root_path = PathBuf::from("resolver/snapshots/source/source/main.omg");

        assert_eq!(retained, Path::new("authored/project/build"));
        assert_eq!(options.build_dir(), retained);
    }

    #[test]
    fn retained_explicit_build_dir_remains_exact() {
        let explicit = PathBuf::from("../exact-artifacts");
        let mut options = CompileOptions {
            root_path: PathBuf::from("authored/project/main.omg"),
            build_dir: Some(explicit.clone()),
            target_name: None,
        };

        let retained = options.retain_build_dir();
        options.root_path = PathBuf::from("resolver/snapshots/source/source/main.omg");

        assert_eq!(retained, explicit);
        assert_eq!(options.build_dir, Some(explicit));
    }

    #[test]
    fn staging_identity_joins_spelling_aliases() {
        let identity = staging_directory_identity(Path::new("aliased-out"));
        assert_eq!(
            identity,
            staging_directory_identity(Path::new("./aliased-out"))
        );
        assert_eq!(
            identity,
            staging_directory_identity(Path::new("staging/../aliased-out"))
        );
        assert_eq!(
            identity,
            staging_directory_identity(Path::new("./deeper/.././aliased-out"))
        );
        assert_eq!(
            identity,
            staging_directory_identity(&std::env::current_dir().unwrap().join("aliased-out"))
        );
    }

    #[test]
    fn staging_identity_keeps_distinct_directories_distinct() {
        assert_ne!(
            staging_directory_identity(Path::new("out")),
            staging_directory_identity(Path::new("other"))
        );
        assert_ne!(
            staging_directory_identity(Path::new("out")),
            staging_directory_identity(Path::new("out/child"))
        );
        assert_ne!(
            staging_directory_identity(Path::new("out")),
            staging_directory_identity(Path::new("other/../out-nope"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn staging_identity_joins_symlink_aliases_on_existing_prefixes() {
        let root =
            std::env::temp_dir().join(format!("omega-build-dir-identity-{}", std::process::id()));
        let real = root.join("real").join("staging");
        std::fs::create_dir_all(&real).unwrap();
        std::os::unix::fs::symlink(root.join("real"), root.join("link")).unwrap();

        assert_eq!(
            staging_directory_identity(&root.join("link").join("staging")),
            staging_directory_identity(&real)
        );
        // A tail that does not exist yet still lands on the resolved prefix.
        assert_eq!(
            staging_directory_identity(&root.join("link").join("new").join("dir")),
            staging_directory_identity(&real.join("..").join("new").join("dir"))
        );

        std::fs::remove_dir_all(&root).unwrap();
    }
}
