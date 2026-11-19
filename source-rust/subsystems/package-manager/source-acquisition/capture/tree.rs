//! A package-relative source tree with explicit parents and contained link resolution.

use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsString;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, record_storage::Record)]
pub enum CapturedSourceNode {
    Directory,
    File { bytes: Arc<[u8]>, executable: bool },
    Symlink { target: PathBuf },
}

#[derive(Debug, Default, PartialEq, Eq, record_storage::Record)]
pub struct CapturedSourceTree {
    entries: BTreeMap<PathBuf, CapturedSourceNode>,
}

impl CapturedSourceTree {
    pub fn new(
        entries: impl IntoIterator<Item = (PathBuf, CapturedSourceNode)>,
    ) -> io::Result<Self> {
        let mut captured_entries = BTreeMap::new();
        for (path, node) in entries {
            validate_entry_path(&path)?;
            if captured_entries.insert(path.clone(), node).is_some() {
                return Err(path_error(
                    io::ErrorKind::AlreadyExists,
                    &path,
                    "duplicate captured entry",
                ));
            }
        }
        for path in captured_entries.keys() {
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
                && !matches!(
                    captured_entries.get(parent),
                    Some(CapturedSourceNode::Directory)
                )
            {
                return Err(path_error(
                    io::ErrorKind::InvalidInput,
                    path,
                    "captured parent is not an explicit directory",
                ));
            }
        }
        Ok(Self {
            entries: captured_entries,
        })
    }

    pub fn entries(&self) -> impl Iterator<Item = (&Path, &CapturedSourceNode)> {
        self.entries
            .iter()
            .map(|(path, node)| (path.as_path(), node))
    }

    pub fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        let mut remaining = relative_components(path)?;
        let mut lexical_depth = 0usize;
        for component in &remaining {
            if component == ".." {
                lexical_depth = lexical_depth.checked_sub(1).ok_or_else(|| {
                    path_error(
                        io::ErrorKind::PermissionDenied,
                        path,
                        "path escapes captured root",
                    )
                })?;
            } else {
                lexical_depth += 1;
            }
        }
        let mut resolved = PathBuf::new();
        let mut link_traversals = 0usize;
        while let Some(component) = remaining.pop_front() {
            if component == ".." {
                if !resolved.pop() {
                    return Err(path_error(
                        io::ErrorKind::PermissionDenied,
                        path,
                        "path escapes captured root",
                    ));
                }
                continue;
            }
            resolved.push(component);
            match self.entries.get(&resolved) {
                Some(CapturedSourceNode::Directory) => {}
                Some(CapturedSourceNode::File { .. }) => {
                    if !remaining.is_empty() {
                        return Err(path_error(
                            io::ErrorKind::NotADirectory,
                            &resolved,
                            "captured path component is a file",
                        ));
                    }
                }
                Some(CapturedSourceNode::Symlink { target }) => {
                    link_traversals += 1;
                    if link_traversals > 64 {
                        return Err(path_error(
                            io::ErrorKind::InvalidData,
                            path,
                            "captured symlink cycle or traversal limit exceeded",
                        ));
                    }
                    if target.as_os_str().is_empty() {
                        return Err(path_error(
                            io::ErrorKind::InvalidInput,
                            &resolved,
                            "captured symlink target is empty",
                        ));
                    }
                    let mut target_components = relative_components(target).map_err(|_| {
                        path_error(
                            io::ErrorKind::PermissionDenied,
                            &resolved,
                            "captured symlink target is not a contained relative path",
                        )
                    })?;
                    resolved.pop();
                    target_components.append(&mut remaining);
                    remaining = target_components;
                }
                None => {
                    return Err(path_error(
                        io::ErrorKind::NotFound,
                        &resolved,
                        "captured entry is absent",
                    ));
                }
            }
        }
        Ok(resolved)
    }

    pub fn exists(&self, path: &Path) -> bool {
        self.canonicalize(path).is_ok()
    }

    pub fn is_file(&self, path: &Path) -> bool {
        self.canonicalize(path).is_ok_and(|resolved| {
            matches!(
                self.entries.get(&resolved),
                Some(CapturedSourceNode::File { .. })
            )
        })
    }

    pub fn is_dir(&self, path: &Path) -> bool {
        self.canonicalize(path).is_ok_and(|resolved| {
            resolved.as_os_str().is_empty()
                || matches!(
                    self.entries.get(&resolved),
                    Some(CapturedSourceNode::Directory)
                )
        })
    }

    pub fn read_to_string(&self, path: &Path) -> io::Result<Arc<str>> {
        let resolved = self.canonicalize(path)?;
        match self.entries.get(&resolved) {
            Some(CapturedSourceNode::File { bytes, .. }) => {
                let text = std::str::from_utf8(bytes).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "stream did not contain valid UTF-8",
                    )
                })?;
                Ok(Arc::from(text))
            }
            _ => Err(path_error(
                io::ErrorKind::IsADirectory,
                path,
                "captured source is a directory",
            )),
        }
    }
}

fn validate_entry_path(path: &Path) -> io::Result<()> {
    let components = relative_components(path)?;
    if components.is_empty() || components.iter().any(|component| component == "..") {
        return Err(path_error(
            io::ErrorKind::InvalidInput,
            path,
            "captured entry path must be nonempty and canonical",
        ));
    }
    let canonical: PathBuf = components.into_iter().collect();
    // Package-relative paths are authored with `/` on every host
    // (`targets/linux_arm64`), and on Windows that names the same entry as
    // the `\` spelling the components rebuild. Compare the given spelling
    // with its separators made native, so a portable spelling is canonical
    // there too while `a//b` and `a/./b` are still refused.
    let given = if cfg!(windows) {
        PathBuf::from(path.to_string_lossy().replace('/', "\\"))
    } else {
        path.to_path_buf()
    };
    if canonical.as_os_str() != given.as_os_str() {
        return Err(path_error(
            io::ErrorKind::InvalidInput,
            path,
            "captured entry path is not canonical",
        ));
    }
    Ok(())
}

fn relative_components(path: &Path) -> io::Result<VecDeque<OsString>> {
    if path.as_os_str().as_encoded_bytes().contains(&0) {
        return Err(path_error(
            io::ErrorKind::InvalidInput,
            path,
            "invalid captured path spelling",
        ));
    }
    path.components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(Ok(name.to_os_string())),
            Component::ParentDir => Some(Ok(OsString::from(".."))),
            Component::CurDir => None,
            Component::RootDir | Component::Prefix(_) => Some(Err(path_error(
                io::ErrorKind::InvalidInput,
                path,
                "captured path must be relative",
            ))),
        })
        .collect()
}

fn path_error(kind: io::ErrorKind, path: &Path, message: &str) -> io::Error {
    io::Error::new(kind, format!("{message}: {}", path.display()))
}

#[cfg(test)]
mod tests {
    /// A package-relative path spelled with `/` is canonical on every host,
    /// and a spelling with an empty or `.` component never is.
    #[test]
    fn a_portable_entry_path_is_canonical_and_a_redundant_one_is_not() {
        use super::validate_entry_path;
        use std::path::Path;
        assert!(validate_entry_path(Path::new("targets/linux_arm64")).is_ok());
        assert!(validate_entry_path(Path::new("main.omg")).is_ok());
        for redundant in [
            "targets//linux_arm64",
            "targets/./linux_arm64",
            "targets/",
            "../x",
        ] {
            assert!(
                validate_entry_path(Path::new(redundant)).is_err(),
                "{redundant} must be refused"
            );
        }
    }
}
