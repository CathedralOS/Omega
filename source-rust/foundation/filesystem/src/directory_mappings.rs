//! Guest paths resolve against the directory handles granted for this invocation.
//! A more specific mount owns its subtree, including missing files.

use crate::bound_directory::BoundDirectory;
use std::collections::BTreeMap;
use std::io;
use std::path::{Component, Path};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectoryAccess {
    ReadOnly,
    ReadWrite,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DirectoryMappings(BTreeMap<String, DirectoryMapping>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryMapping {
    backing: BoundDirectory,
    access: DirectoryAccess,
}

impl DirectoryMappings {
    /// Bind the host directory now. A subsequent rename cannot redirect the grant.
    pub fn grant(&mut self, host: &Path, guest: &str, access: DirectoryAccess) -> io::Result<()> {
        let guest_path = guest.strip_prefix('/').ok_or(io::ErrorKind::InvalidInput)?;
        if !guest_path.is_empty() && !valid_components(guest_path) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "guest directory must be an absolute canonical path",
            ));
        }
        let std::collections::btree_map::Entry::Vacant(slot) = self.0.entry(guest.into()) else {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "duplicate guest directory",
            ));
        };
        slot.insert(DirectoryMapping {
            backing: BoundDirectory::bind(host)?,
            access,
        });
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn resolve_file<'path>(
        &self,
        path: &'path [u8],
    ) -> io::Result<(&DirectoryMapping, &'path [u8])> {
        let (mapping, relative) = self.resolve_entry(path)?;
        if relative.is_empty() {
            return Err(io::ErrorKind::IsADirectory.into());
        }
        Ok((mapping, relative))
    }

    /// An empty suffix selects the root of the issued directory.
    pub fn resolve_entry<'path>(
        &self,
        path: &'path [u8],
    ) -> io::Result<(&DirectoryMapping, &'path [u8])> {
        let path = std::str::from_utf8(path).map_err(|_| io::ErrorKind::InvalidInput)?;
        let relative = path.strip_prefix('/').ok_or(io::ErrorKind::InvalidInput)?;
        if !relative.is_empty() && !valid_components(relative) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        if let Some(mapping) = self.0.get(path) {
            return Ok((mapping, &[]));
        }
        for (offset, _) in path.rmatch_indices('/') {
            let mount = if offset == 0 { "/" } else { &path[..offset] };
            if let Some(mapping) = self.0.get(mount) {
                return Ok((mapping, &path.as_bytes()[offset + 1..]));
            }
        }
        Err(io::ErrorKind::PermissionDenied.into())
    }
}

impl DirectoryMapping {
    pub fn access(&self) -> DirectoryAccess {
        self.access
    }

    /// Hand the matching-host filesystem adapter a retained parent and one leaf.
    /// The adapter must enforce this mapping's access and avoid following the leaf.
    #[cfg(target_os = "linux")]
    pub fn bound_parent<'a>(&self, relative: &'a [u8]) -> io::Result<(std::fs::File, &'a str)> {
        self.backing.bound_parent(relative)
    }

    pub fn open_read(&self, relative: &[u8]) -> io::Result<std::fs::File> {
        self.backing.open_read(relative)
    }
    pub fn open_write(&self, relative: &[u8]) -> io::Result<std::fs::File> {
        if self.access == DirectoryAccess::ReadOnly {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        self.backing.open_write(relative)
    }
}

pub(super) fn valid_components(spelling: &str) -> bool {
    !spelling.as_bytes().contains(&0)
        && !spelling.contains('\\')
        && spelling.split('/').all(|part| {
            !part.is_empty()
                && (!cfg!(windows) || !part.contains(':'))
                && matches!(
                    Path::new(part).components().next(),
                    Some(Component::Normal(_))
                )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        io::{Read, Write},
    };

    #[test]
    fn nested_directory_owns_the_path_even_if_only_the_parent_contains_the_file() {
        let temp =
            crate::temporary_directory::TemporaryDirectory::create("mapped-directories", false)
                .unwrap();
        let parent = temp.path().join("parent");
        let child = temp.path().join("child");
        fs::create_dir_all(parent.join("sdk")).unwrap();
        fs::create_dir(&child).unwrap();
        fs::write(parent.join("sdk/parent-only"), b"wrong").unwrap();
        let mut mounts = DirectoryMappings::default();
        mounts
            .grant(&parent, "/", DirectoryAccess::ReadWrite)
            .unwrap();
        mounts
            .grant(&child, "/sdk", DirectoryAccess::ReadOnly)
            .unwrap();
        let (nested, path) = mounts.resolve_file(b"/sdk/parent-only").unwrap();
        assert_eq!(
            nested.open_read(path).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        assert_eq!(
            nested.open_write(path).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        fs::write(child.join("config"), b"first").unwrap();
        let (nested, path) = mounts.resolve_file(b"/sdk/config").unwrap();
        fs::write(child.join("config"), b"second").unwrap();
        let mut text = String::new();
        nested
            .open_read(path)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, "second");
        let (writable, path) = mounts.resolve_file(b"/file").unwrap();
        writable
            .open_write(path)
            .unwrap()
            .write_all(b"first")
            .unwrap();
        writable
            .open_write(path)
            .unwrap()
            .write_all(b"two")
            .unwrap();
        assert_eq!(fs::read(parent.join("file")).unwrap(), b"two");
        assert!(mounts.resolve_file(b"/sdk/../file").is_err());
    }
}
