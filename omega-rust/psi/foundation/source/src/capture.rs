/*
We implement the immutable captured source tree (`CapturedSourceTree`,
`CapturedSourceNode`), the invocation-level `SourceFileSystem` router, and the
non-following `SourceSnapshot` inventory protocol.

`CapturedSourceTree` holds a closed, canonical relative namespace with explicit parent
directory coverage and bounded relative-symlink resolution for compiler lookups.
`SourceFileSystem` mounts immutable trees at canonical absolute roots so missing captured
entries never reopen host files, while resolving host link prefixes freshly on every route
so retargeted host aliases cannot bypass a mounted capture. `SourceSnapshot` provides
the non-following inventory queries consumed by Build without copying storage or granting
host access.
*/

use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsString;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, compiler_storage::Record)]
pub enum CapturedSourceNode {
    Directory,
    File { bytes: Arc<[u8]>, executable: bool },
    Symlink { target: PathBuf },
}

#[derive(Debug, Default, PartialEq, Eq, compiler_storage::Record)]
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

#[derive(Clone, Debug, Default, PartialEq, Eq, compiler_storage::Record)]
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
                    .filter_map(|(path, node)| {
                        (path.starts_with(&relative)
                            && matches!(node, CapturedSourceNode::File { .. }))
                        .then(|| root.join(path))
                    })
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceSnapshotEntry<'storage> {
    Directory,
    File(&'storage [u8]),
    Symlink(&'storage [u8]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceSnapshotMetadata {
    Directory,
    File { byte_length: u64 },
    Symlink { target_byte_length: u64 },
}

pub trait SourceSnapshotStorage: Send + Sync {
    fn entry(&self, path: &[u8]) -> Option<SourceSnapshotEntry<'_>>;
    fn entries(&self) -> Box<dyn Iterator<Item = (&[u8], SourceSnapshotEntry<'_>)> + '_>;
}

#[derive(Clone)]
pub struct SourceSnapshot(Arc<dyn SourceSnapshotStorage>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceSnapshotError {
    InvalidPath,
    NotFound,
    NotDirectory,
    NotFile,
    NotSymlink,
    InvalidRange,
}

impl SourceSnapshot {
    pub fn new(storage: impl SourceSnapshotStorage + 'static) -> Self {
        Self(Arc::new(storage))
    }

    pub fn lookup(&self, path: &[u8]) -> Result<SourceSnapshotEntry<'_>, SourceSnapshotError> {
        if !source_snapshot_path_is_canonical(path) {
            return Err(SourceSnapshotError::InvalidPath);
        }
        for (index, byte) in path.iter().enumerate() {
            if *byte == b'/' {
                match self.0.entry(&path[..index]) {
                    Some(SourceSnapshotEntry::Directory) => {}
                    Some(_) => return Err(SourceSnapshotError::NotDirectory),
                    None => return Err(SourceSnapshotError::NotFound),
                }
            }
        }
        if path.is_empty() {
            return Ok(SourceSnapshotEntry::Directory);
        }
        self.0.entry(path).ok_or(SourceSnapshotError::NotFound)
    }

    pub fn metadata(&self, path: &[u8]) -> Result<SourceSnapshotMetadata, SourceSnapshotError> {
        Ok(match self.lookup(path)? {
            SourceSnapshotEntry::Directory => SourceSnapshotMetadata::Directory,
            SourceSnapshotEntry::File(bytes) => SourceSnapshotMetadata::File {
                byte_length: bytes.len() as u64,
            },
            SourceSnapshotEntry::Symlink(target) => SourceSnapshotMetadata::Symlink {
                target_byte_length: target.len() as u64,
            },
        })
    }

    pub fn read(&self, path: &[u8], offset: u64, count: u64) -> Result<&[u8], SourceSnapshotError> {
        let SourceSnapshotEntry::File(bytes) = self.lookup(path)? else {
            return Err(SourceSnapshotError::NotFile);
        };
        offset
            .checked_add(count)
            .ok_or(SourceSnapshotError::InvalidRange)?;
        let offset = usize::try_from(offset).map_err(|_| SourceSnapshotError::InvalidRange)?;
        if offset > bytes.len() {
            return Err(SourceSnapshotError::InvalidRange);
        }
        let count = usize::try_from(count)
            .unwrap_or(usize::MAX)
            .min(bytes.len() - offset);
        Ok(&bytes[offset..offset + count])
    }

    pub fn read_link(&self, path: &[u8]) -> Result<&[u8], SourceSnapshotError> {
        match self.lookup(path)? {
            SourceSnapshotEntry::Symlink(target) => Ok(target),
            _ => Err(SourceSnapshotError::NotSymlink),
        }
    }

    pub fn list(
        &self,
        path: &[u8],
    ) -> Result<Vec<(&[u8], SourceSnapshotEntry<'_>)>, SourceSnapshotError> {
        if self.lookup(path)? != SourceSnapshotEntry::Directory {
            return Err(SourceSnapshotError::NotDirectory);
        }
        let mut children = Vec::new();
        for (member, entry) in self.0.entries() {
            let suffix = if path.is_empty() {
                member
            } else if member.starts_with(path) && member.get(path.len()) == Some(&b'/') {
                &member[path.len() + 1..]
            } else {
                continue;
            };
            if !suffix.is_empty() && !suffix.contains(&b'/') {
                children.push((suffix, entry));
            }
        }
        children.sort_unstable_by(|left, right| left.0.cmp(right.0));
        Ok(children)
    }
}

pub fn source_snapshot_path_is_canonical(path: &[u8]) -> bool {
    std::str::from_utf8(path).is_ok()
        && !path.contains(&0)
        && !path.contains(&b'\\')
        && !(path.len() >= 2 && path[1] == b':')
        && (path.is_empty()
            || path
                .split(|byte| *byte == b'/')
                .all(|component| !component.is_empty() && component != b"." && component != b".."))
}

impl std::fmt::Debug for SourceSnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceSnapshot")
            .field("entries", &self.0.entries().count())
            .finish()
    }
}

impl PartialEq for SourceSnapshot {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.0.entries().eq(other.0.entries())
    }
}

impl Eq for SourceSnapshot {}

#[cfg(test)]
mod tests {
    use super::{CapturedSourceNode, CapturedSourceTree, SourceFileSystem};
    #[cfg(unix)]
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;

    struct TemporaryDirectory(PathBuf);

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temporary_directory() -> TemporaryDirectory {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("omega-source-view-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        TemporaryDirectory(std::fs::canonicalize(&path).unwrap())
    }

    fn captured_file(text: &str) -> CapturedSourceNode {
        CapturedSourceNode::File {
            bytes: Arc::from(text.as_bytes()),
            executable: false,
        }
    }

    #[test]
    fn joining_captures_keeps_shared_snapshots_and_refuses_conflicts_atomically() {
        let directory = temporary_directory();
        let source = |text| {
            Arc::new(
                CapturedSourceTree::new([(PathBuf::from("main.omg"), captured_file(text))])
                    .unwrap(),
            )
        };
        let first = directory.0.join("first");
        let second = directory.0.join("second");
        let mut destination = SourceFileSystem::new();
        destination
            .add_capture(first.clone(), source("first snapshot"))
            .unwrap();
        let mut supplied = SourceFileSystem::new();
        supplied
            .add_capture(first.clone(), source("first snapshot"))
            .unwrap();
        supplied
            .add_capture(second.clone(), source("second snapshot"))
            .unwrap();
        destination.join_captures(&supplied).unwrap();
        assert_eq!(
            destination
                .read_to_string(&second.join("main.omg"))
                .unwrap()
                .as_ref(),
            "second snapshot"
        );
        let saved = destination.clone();
        let mut conflict = SourceFileSystem::new();
        conflict
            .add_capture(directory.0.join("additional"), source("must not leak"))
            .unwrap();
        conflict
            .add_capture(second, source("changed snapshot"))
            .unwrap();
        assert!(destination.join_captures(&conflict).is_err());
        assert_eq!(destination, saved);
        assert!(!destination.exists(&first.join("not captured.omg")));
    }

    #[cfg(unix)]
    #[test]
    fn host_links_into_a_capture_read_captured_bytes_on_every_repeated_route() {
        let directory = temporary_directory();
        let project = directory.0.join("project");
        std::fs::create_dir(&project).unwrap();
        std::fs::write(project.join("main.omg"), "on disk").unwrap();
        std::fs::write(directory.0.join("outside.omg"), "outside").unwrap();
        let alias = directory.0.join("alias");
        std::os::unix::fs::symlink(&project, &alias).unwrap();

        let tree = CapturedSourceTree::new([
            (PathBuf::from("main.omg"), captured_file("captured")),
            (
                PathBuf::from("only_captured.omg"),
                captured_file("not on disk"),
            ),
        ])
        .unwrap();
        let mut view = SourceFileSystem::new();
        view.add_capture(project.clone(), Arc::new(tree)).unwrap();

        for _ in 0..2 {
            let read = |path: &Path| view.read_to_string(path).unwrap().to_string();
            assert_eq!(read(&alias.join("main.omg")), "captured");
            assert_eq!(read(&alias.join("only_captured.omg")), "not on disk");
            assert_eq!(read(&project.join("main.omg")), "captured");
            assert_eq!(read(&directory.0.join("outside.omg")), "outside");
            assert!(!view.exists(&alias.join("missing.omg")));
        }

        assert_eq!(view.clone(), view);
        assert_ne!(view, SourceFileSystem::new());
    }

    #[cfg(unix)]
    fn changing_host_alias_enters_capture(start_with_link: bool) {
        let directory = temporary_directory();
        let project = directory.0.join("project");
        let outside = directory.0.join("outside");
        let alias = directory.0.join("alias");
        std::fs::create_dir(&project).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(project.join("main.omg"), "live capture disk").unwrap();
        std::fs::write(project.join("missing.omg"), "disk only").unwrap();
        std::fs::write(outside.join("main.omg"), "outside").unwrap();
        std::fs::write(outside.join("missing.omg"), "outside missing").unwrap();
        if start_with_link {
            std::os::unix::fs::symlink(&outside, &alias).unwrap();
        } else {
            std::fs::create_dir(&alias).unwrap();
            std::fs::write(alias.join("main.omg"), "outside").unwrap();
            std::fs::write(alias.join("missing.omg"), "outside missing").unwrap();
        }
        let tree = CapturedSourceTree::new([
            (
                PathBuf::from("main.omg"),
                captured_file("captured immutable"),
            ),
            (
                PathBuf::from("captured_directory"),
                CapturedSourceNode::Directory,
            ),
        ])
        .unwrap();
        let mut view = SourceFileSystem::new();
        view.add_capture(project.clone(), Arc::new(tree)).unwrap();
        let clone = view.clone();
        assert_eq!(
            view.read_to_string(&alias.join("main.omg"))
                .unwrap()
                .as_ref(),
            "outside"
        );
        assert!(view.exists(&alias.join("missing.omg")));
        if start_with_link {
            std::fs::remove_file(&alias).unwrap();
        } else {
            std::fs::remove_dir_all(&alias).unwrap();
        }
        std::os::unix::fs::symlink(&project, &alias).unwrap();

        for view in [&view, &clone] {
            assert_eq!(
                view.read_to_string(&alias.join("main.omg"))
                    .unwrap()
                    .as_ref(),
                "captured immutable"
            );
            assert_eq!(
                view.canonicalize(&alias.join("main.omg")).unwrap(),
                project.join("main.omg")
            );
            assert!(view.is_file(&alias.join("main.omg")));
            assert!(view.is_dir(&alias.join("captured_directory")));
            assert!(!view.exists(&alias.join("missing.omg")));
            assert!(!view.is_file(&alias.join("missing.omg")));
            assert_eq!(
                view.read_to_string(&alias.join("missing.omg"))
                    .unwrap_err()
                    .kind(),
                std::io::ErrorKind::NotFound
            );
            assert_eq!(
                view.canonicalize(&alias.join("missing.omg"))
                    .unwrap_err()
                    .kind(),
                std::io::ErrorKind::NotFound
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn replaced_host_directory_cannot_bypass_a_capture() {
        changing_host_alias_enters_capture(false);
    }

    #[cfg(unix)]
    #[test]
    fn retargeted_host_link_cannot_bypass_a_capture() {
        changing_host_alias_enters_capture(true);
    }
}
