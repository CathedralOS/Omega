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
