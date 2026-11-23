//! Bounded, capability-relative capture of a quiescent local package.
//! Keep opening observations until the final sweep, including the names of
//! excluded members. Only canonical nodes and bytes enter declaration identity.
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, Metadata, OpenOptions};
use std::collections::{BTreeMap, VecDeque};
use std::ffi::{OsStr, OsString};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

pub(super) type Snapshot = BTreeMap<PathBuf, Node>;
pub(super) enum Node {
    Directory,
    File { bytes: Arc<[u8]>, executable: bool },
    Link { target: PathBuf },
}

// The existing local source acquisition defaults.
const MAX_ENTRIES: usize = 4096;
const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_DEPTH: usize = 64;

struct DirectoryObservation {
    metadata: Metadata,
    names: Vec<OsString>,
}

#[derive(Default)]
struct Capture {
    nodes: Snapshot,
    members: BTreeMap<PathBuf, Metadata>,
    directories: BTreeMap<PathBuf, DirectoryObservation>,
    bytes: u64,
}

fn unchanged(
    before: &Metadata,
    after: &Metadata,
    path: &Path,
) -> io::Result<()> {
    #[cfg(windows)]
    let same_identity = {
        use cap_primitives::fs::_WindowsByHandle;
        matches!((before.volume_serial_number(), before.file_index(), after.volume_serial_number(), after.file_index()),
            (Some(v), Some(i), Some(w), Some(j)) if (v,i) == (w,j))
    };
    #[cfg(unix)]
    let same_identity = {
        use cap_fs_ext::{MetadataExt, OsMetadataExt};
        MetadataExt::dev(before) == MetadataExt::dev(after)
            && MetadataExt::ino(before) == MetadataExt::ino(after)
            && before.ctime() == after.ctime()
            && before.ctime_nsec() == after.ctime_nsec()
    };
    if !same_identity
        || before.file_type() != after.file_type()
        || before.len() != after.len()
        || before.permissions() != after.permissions()
        || before.modified()? != after.modified()?
    {
        return Err(io::Error::other(format!("package source changed during capture at {}", path.display())));
    }
    Ok(())
}

fn excluded(
    parent: &Path,
    name: &OsStr,
) -> bool {
    name == ".git" || (parent.as_os_str().is_empty() && (name == "build" || name.as_encoded_bytes().eq_ignore_ascii_case(b"omega.lock")))
}

fn names(dir: &Dir) -> io::Result<Vec<OsString>> {
    let mut names = Vec::new();
    for entry in dir.entries()? {
        if names.len() >= MAX_ENTRIES + 3 {
            return Err(io::Error::other("package source exceeds capture entry limit"));
        }
        names.push(entry?.file_name());
    }
    names.sort_unstable();
    Ok(names)
}

impl Capture {
    fn directory(
        &mut self,
        dir: &Dir,
        relative: &Path,
        depth: usize,
    ) -> io::Result<()> {
        if depth > MAX_DEPTH {
            return Err(io::Error::other("package source exceeds capture depth limit"));
        }
        let metadata = dir.dir_metadata()?;
        let names = names(dir)?;
        for name in names.iter().filter(|name| !excluded(relative, name)) {
            if self.members.len() >= MAX_ENTRIES {
                return Err(io::Error::other("package source exceeds capture entry limit"));
            }
            let path = relative.join(name);
            let opening = dir.symlink_metadata(name)?;
            self.members.insert(path.clone(), opening.clone());
            let node = if opening.file_type().is_symlink() {
                let target = dir.read_link_contents(name)?;
                self.account(super::os_bytes(target.as_os_str())?.len() as u64)?;
                Node::Link { target }
            } else if opening.is_dir() {
                let child = dir.open_dir_nofollow(name)?;
                unchanged(&opening, &child.dir_metadata()?, &path)?;
                self.directory(&child, &path, depth + 1)?;
                Node::Directory
            } else if opening.is_file() {
                self.account(opening.len())?;
                let mut options = OpenOptions::new();
                options.read(true).follow(FollowSymlinks::No);
                let file = dir.open_with(name, &options)?;
                unchanged(&opening, &file.metadata()?, &path)?;
                let mut bytes = Vec::new();
                file.take(opening.len() + 1).read_to_end(&mut bytes)?;
                if bytes.len() as u64 != opening.len() {
                    return Err(io::Error::other("package source length changed during capture"));
                }
                #[cfg(unix)]
                let executable = cap_fs_ext::OsMetadataExt::mode(&opening) & 0o111 != 0;
                #[cfg(not(unix))]
                let executable = false;
                Node::File {
                    bytes: Arc::from(bytes),
                    executable,
                }
            } else {
                return Err(io::Error::other(format!("unsupported package member {}", path.display())));
            };
            self.nodes.insert(path, node);
        }
        self.directories
            .insert(relative.to_owned(), DirectoryObservation { metadata, names });
        Ok(())
    }

    fn account(
        &mut self,
        count: u64,
    ) -> io::Result<()> {
        if count > MAX_BYTES - self.bytes {
            return Err(io::Error::other("package source exceeds capture byte limit"));
        }
        self.bytes += count;
        Ok(())
    }

    fn verify(
        &self,
        dir: &Dir,
        path: &Path,
    ) -> io::Result<()> {
        let observation = &self.directories[path];
        unchanged(&observation.metadata, &dir.dir_metadata()?, path)?;
        if names(dir)? != observation.names {
            return Err(io::Error::other(format!("package membership changed during capture at {}", path.display())));
        }
        for name in observation.names.iter().filter(|name| !excluded(path, name)) {
            let child = path.join(name);
            unchanged(&self.members[&child], &dir.symlink_metadata(name)?, &child)?;
            if self.directories.contains_key(&child) {
                self.verify(&dir.open_dir_nofollow(name)?, &child)?;
            }
        }
        Ok(())
    }
}

pub(super) fn local_package(root: &Path) -> io::Result<Snapshot> {
    let directory = Dir::open_ambient_dir(root, cap_std::ambient_authority())?;
    let mut capture = Capture::default();
    capture.directory(&directory, Path::new(""), 0)?;
    capture.verify(&directory, Path::new(""))?;
    for (path, node) in &capture.nodes {
        if matches!(node, Node::Link { .. }) {
            resolve(&capture.nodes, path)?;
        }
    }
    Ok(capture.nodes)
}

fn components(path: &Path) -> io::Result<VecDeque<OsString>> {
    path.components()
        .filter(|part| *part != Component::CurDir)
        .map(|part| match part {
            Component::Normal(name) => Ok(name.to_owned()),
            Component::ParentDir => Ok(OsString::from("..")),
            _ => Err(io::Error::other("package link or input escapes its root")),
        })
        .collect()
}

/// Resolve strictly in the retained namespace. Absolute links, dangling links,
/// links through omitted members and cycles cannot grant access to host paths.
pub(super) fn resolve(
    snapshot: &Snapshot,
    path: &Path,
) -> io::Result<PathBuf> {
    let mut remaining = components(path)?;
    let mut resolved = PathBuf::new();
    let mut links = 0;
    while let Some(next) = remaining.pop_front() {
        if next == ".." {
            if !resolved.pop() {
                return Err(io::Error::other("package link or input escapes its root"));
            }
            continue;
        }
        resolved.push(next);
        match snapshot.get(&resolved) {
            Some(Node::Directory) => {}
            Some(Node::File { .. }) if remaining.is_empty() => {}
            Some(Node::File { .. }) => return Err(io::Error::other("package path crosses a regular file")),
            Some(Node::Link { target }) => {
                links += 1;
                if links > MAX_DEPTH {
                    return Err(io::Error::other("package link cycle or traversal limit exceeded"));
                }
                let mut target = components(target)?;
                target.append(&mut remaining);
                remaining = target;
                resolved.pop();
            }
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("{} is absent from captured package", resolved.display()),
                ));
            }
        }
    }
    Ok(resolved)
}
