//! Route source lookups through the immutable trees mounted for this acquisition.

use crate::{CapturedSourceNode, CapturedSourceTree};
use std::collections::BTreeMap;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug, Default, PartialEq, Eq, record_storage::Record)]
pub struct SourceFileSystem {
    captures: BTreeMap<PathBuf, Arc<CapturedSourceTree>>,
}

impl SourceFileSystem {
    pub fn has_captured_namespace(&self, canonical_root: &Path) -> bool {
        canonical_root.is_absolute()
            && self
                .captures
                .keys()
                .any(|root| canonical_root.starts_with(root))
    }

    pub fn join_captures(&mut self, other: &Self) -> io::Result<()> {
        for (root, tree) in &other.captures {
            if self
                .captures
                .get(root)
                .is_some_and(|existing| existing != tree)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("source captures disagree at {}", root.display()),
                ));
            }
        }
        for (root, tree) in &other.captures {
            self.captures
                .entry(root.clone())
                .or_insert_with(|| Arc::clone(tree));
        }
        Ok(())
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_capture(&mut self, root: PathBuf, tree: Arc<CapturedSourceTree>) -> io::Result<()> {
        let canonical: PathBuf = root
            .components()
            .map(|component| component.as_os_str())
            .collect();
        if !root.is_absolute()
            || root
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
            || canonical.as_os_str() != root.as_os_str()
            || root.as_os_str().as_encoded_bytes().contains(&0)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "capture root must be canonical and absolute: {}",
                    root.display()
                ),
            ));
        }
        if self.captures.contains_key(&root) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("capture root is already mounted: {}", root.display()),
            ));
        }
        self.captures.insert(root, tree);
        Ok(())
    }

    pub fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        match self.route(path)? {
            SourcePath::Captured {
                root,
                tree,
                relative,
            } => {
                let resolved = tree.canonicalize(&relative)?;
                Ok(if resolved.as_os_str().is_empty() {
                    root.to_path_buf()
                } else {
                    root.join(resolved)
                })
            }
            SourcePath::Physical(path) => std::fs::canonicalize(path),
        }
    }

    pub fn exists(&self, path: &Path) -> bool {
        match self.route(path) {
            Ok(SourcePath::Captured { tree, relative, .. }) => tree.exists(&relative),
            Ok(SourcePath::Physical(path)) => path.exists(),
            Err(_) => false,
        }
    }

    pub fn is_file(&self, path: &Path) -> bool {
        match self.route(path) {
            Ok(SourcePath::Captured { tree, relative, .. }) => tree.is_file(&relative),
            Ok(SourcePath::Physical(path)) => path.is_file(),
            Err(_) => false,
        }
    }

    pub fn is_dir(&self, path: &Path) -> bool {
        match self.route(path) {
            Ok(SourcePath::Captured { tree, relative, .. }) => tree.is_dir(&relative),
            Ok(SourcePath::Physical(path)) => path.is_dir(),
            Err(_) => false,
        }
    }

    pub fn read_to_string(&self, path: &Path) -> io::Result<Arc<str>> {
        match self.route(path)? {
            SourcePath::Captured { tree, relative, .. } => tree.read_to_string(&relative),
            SourcePath::Physical(path) => {
                let bytes = std::fs::read(&path)?;
                crate::read_accounting::record_source_file_read(
                    "frontend_load",
                    &path,
                    bytes.len() as u64,
                );
                let text = std::str::from_utf8(&bytes).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "stream did not contain valid UTF-8",
                    )
                })?;
                Ok(Arc::from(text))
            }
        }
    }

    /// Enumerate regular source members from the already captured namespace.
    /// Logical module discovery must not reopen a live directory or turn an
    /// omitted row into an additional compiler input. Links need no extra rows:
    /// their contained targets already have one canonical inventory entry.
    pub fn captured_files(&self, directory: &Path) -> io::Result<Vec<PathBuf>> {
        match self.route(directory)? {
            SourcePath::Captured {
                root,
                tree,
                relative,
            } => {
                let relative = tree.canonicalize(&relative)?;
                if !tree.is_dir(&relative) {
                    return Err(io::Error::new(
                        io::ErrorKind::NotADirectory,
                        "logical source inventory root is not a directory",
                    ));
                }
                Ok(tree
                    .entries()
                    .filter(|(path, node)| {
                        path.starts_with(&relative)
                            && matches!(node, CapturedSourceNode::File { .. })
                    })
                    .map(|(path, _)| root.join(path))
                    .collect())
            }
            SourcePath::Physical(_) => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "logical source discovery requires an immutable source capture",
            )),
        }
    }

    fn route(&self, path: &Path) -> io::Result<SourcePath<'_>> {
        if self.captures.is_empty() {
            return Ok(SourcePath::Physical(path.to_path_buf()));
        }
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        let mut routed_path = absolute;
        for _ in 0..64 {
            if let Some(capture) = self.route_capture(&routed_path)? {
                return Ok(capture);
            }
            let mut prefix = PathBuf::new();
            let mut components = routed_path.components();
            let mut redirected = None;
            while let Some(component) = components.next() {
                prefix.push(component.as_os_str());
                if let Ok(target) = std::fs::read_link(&prefix) {
                    let mut destination = if target.is_absolute() {
                        target
                    } else {
                        prefix
                            .parent()
                            .ok_or_else(|| {
                                io::Error::new(
                                    io::ErrorKind::InvalidInput,
                                    "host symlink has no parent directory",
                                )
                            })?
                            .join(target)
                    };
                    destination.extend(components.map(|component| component.as_os_str()));
                    redirected = Some(destination);
                    break;
                }
            }
            let Some(destination) = redirected else {
                return Ok(SourcePath::Physical(path.to_path_buf()));
            };
            routed_path = destination;
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "source filesystem alias cycle or traversal limit exceeded: {}",
                path.display()
            ),
        ))
    }

    fn route_capture(&self, absolute: &Path) -> io::Result<Option<SourcePath<'_>>> {
        let mut normalized = PathBuf::new();
        let mut capture: Option<(&PathBuf, &Arc<CapturedSourceTree>)> = None;
        for component in absolute.components() {
            match component {
                Component::CurDir => continue,
                Component::ParentDir => {
                    if capture.is_some_and(|(root, _)| root == &normalized) {
                        return Err(io::Error::new(
                            io::ErrorKind::PermissionDenied,
                            format!("path escapes captured root: {}", absolute.display()),
                        ));
                    }
                    normalized.pop();
                }
                _ => normalized.push(component.as_os_str()),
            }
            if let Some(mounted) = self.captures.get_key_value(&normalized) {
                capture = Some(mounted);
            }
        }
        let Some((root, tree)) = capture else {
            return Ok(None);
        };
        let relative = absolute
            .strip_prefix(root)
            .or_else(|_| normalized.strip_prefix(root))
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?
            .to_path_buf();
        Ok(Some(SourcePath::Captured {
            root,
            tree,
            relative,
        }))
    }
}

enum SourcePath<'a> {
    Captured {
        root: &'a Path,
        tree: &'a CapturedSourceTree,
        relative: PathBuf,
    },
    Physical(PathBuf),
}
