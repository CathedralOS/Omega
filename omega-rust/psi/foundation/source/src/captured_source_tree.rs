//! Immutable source payload and namespace, independent of acquisition and Build.
//!
//! Entries retain exact file bytes and inert link targets. No operation consults
//! the host, and absent entries stay absent. Compiler path operations follow
//! contained relative links; Build consumes `entries()` without following links.
//! Bytes are source payload, not semantic identity or evidence of admission.

use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsString;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

/// One captured entry. Link targets remain data until a compiler lookup follows
/// them; the executable bit is retained payload for the acquiring consumer.
#[derive(Clone, Debug, PartialEq, Eq, compiler_storage::Record)]
pub enum CapturedSourceNode {
    Directory,
    File { bytes: Arc<[u8]>, executable: bool },
    Symlink { target: PathBuf },
}

/// A closed, immutable relative namespace with an implicit directory root.
///
/// Construction validates canonical entry paths and explicit parent directory
/// coverage. Reads neither reopen files nor discover new directory members.
/// Compiler lookups resolve relative symlinks with at most 64 traversals and
/// reject escapes, dangling links and cycles. Consumers implementing Build's
/// non-following protocol must inspect `entries()` instead of compiler lookups.
#[derive(Debug, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CapturedSourceTree {
    entries: BTreeMap<PathBuf, CapturedSourceNode>,
}

impl CapturedSourceTree {
    /// Construct a tree from canonical, nonempty relative entry paths.
    /// Duplicate paths, noncanonical paths and missing/non-directory parents
    /// reject; the root is implicit and must not appear as an entry.
    /// Native path and link spellings are retained without UTF-8 conversion or
    /// portable member-name policy; those restrictions belong to request owners.
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

    /// Iterate exact captured paths and nodes in path order, without following
    /// symlinks or exposing mutable namespace or byte storage.
    pub fn entries(&self) -> impl Iterator<Item = (&Path, &CapturedSourceNode)> {
        self.entries
            .iter()
            .map(|(path, node)| (path.as_path(), node))
    }

    /// Resolve a relative compiler path using captured entries alone. The
    /// implicit root is returned as an empty path. `.` and contained `..` are
    /// accepted for lookup, but are never stored as entry names.
    pub fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        let mut remaining = relative_components(path)?;
        // A lexical escape must reject even if a preceding symlink could make
        // the parent traversal land inside a deeper captured directory.
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

    /// Test compiler-visible membership; invalid paths and unresolved links
    /// return false and never consult a physical filesystem.
    pub fn exists(&self, path: &Path) -> bool {
        self.canonicalize(path).is_ok()
    }

    /// Test the resolved captured entry kind, following contained links only.
    pub fn is_file(&self, path: &Path) -> bool {
        self.canonicalize(path).is_ok_and(|resolved| {
            matches!(
                self.entries.get(&resolved),
                Some(CapturedSourceNode::File { .. })
            )
        })
    }

    /// Test the resolved captured entry kind, including the implicit root.
    pub fn is_dir(&self, path: &Path) -> bool {
        self.canonicalize(path).is_ok_and(|resolved| {
            resolved.as_os_str().is_empty()
                || matches!(
                    self.entries.get(&resolved),
                    Some(CapturedSourceNode::Directory)
                )
        })
    }

    /// Validate captured file bytes as UTF-8 and copy them once into retained
    /// source text. Arbitrary binary assets remain valid captured entries.
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
    if canonical.as_os_str() != path.as_os_str() {
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
