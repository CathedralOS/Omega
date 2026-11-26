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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryMappings<B = BoundDirectory>(BTreeMap<String, DirectoryMapping<B>>);

impl<B> Default for DirectoryMappings<B> {
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryMapping<B = BoundDirectory> {
    backing: B,
    access: DirectoryAccess,
}

impl DirectoryMappings {
    /// Bind the host directory now. A subsequent rename cannot redirect the grant.
    pub fn grant(&mut self, host: &Path, guest: &str, access: DirectoryAccess) -> io::Result<()> {
        self.insert(guest, BoundDirectory::bind(host)?, access)
    }
}

impl<B> DirectoryMappings<B> {
    pub fn map_backings<C>(self, mut convert: impl FnMut(B) -> C) -> DirectoryMappings<C> {
        DirectoryMappings(
            self.0
                .into_iter()
                .map(|(guest, mapping)| {
                    (
                        guest,
                        DirectoryMapping {
                            backing: convert(mapping.backing),
                            access: mapping.access,
                        },
                    )
                })
                .collect(),
        )
    }

    pub fn insert(&mut self, guest: &str, backing: B, access: DirectoryAccess) -> io::Result<()> {
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
            backing,
            access,
        });
        Ok(())
    }

    /// Re-root an existing grant, preserving every mount that shadows part of
    /// its subtree. A read-only projection attenuates all of its descendants.
    pub fn remount(
        &self,
        from: &str,
        to: &str,
        access: DirectoryAccess,
        subtree: impl FnOnce(&B, &[u8]) -> io::Result<B>,
    ) -> io::Result<Self>
    where
        B: Clone,
    {
        let (root, relative) = self.resolve_entry(from.as_bytes())?;
        if access == DirectoryAccess::ReadWrite && root.access == DirectoryAccess::ReadOnly {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        let mut result = Self::default();
        result.insert(to, subtree(&root.backing, relative)?, access)?;
        for (suffix, child) in self.descendants(from) {
            let guest = format!("{}/{suffix}", to.trim_end_matches('/'));
            let child_access = if access == DirectoryAccess::ReadOnly {
                access
            } else {
                child.access
            };
            result.insert(&guest, child.backing.clone(), child_access)?;
        }
        Ok(result)
    }

    pub fn descendants<'a>(
        &'a self,
        from: &'a str,
    ) -> impl Iterator<Item = (&'a str, &'a DirectoryMapping<B>)> + 'a {
        self.0.iter().filter_map(move |(name, child)| {
            let suffix = name
                .strip_prefix(from.trim_end_matches('/'))?
                .strip_prefix('/')?;
            (!suffix.is_empty()).then_some((suffix, child))
        })
    }

    pub fn entries(&self) -> impl Iterator<Item = (&str, &DirectoryMapping<B>)> {
        self.0.iter().map(|(guest, item)| (guest.as_str(), item))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn resolve_file<'path>(
        &self,
        path: &'path [u8],
    ) -> io::Result<(&DirectoryMapping<B>, &'path [u8])> {
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
    ) -> io::Result<(&DirectoryMapping<B>, &'path [u8])> {
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

impl<B> DirectoryMapping<B> {
    pub fn backing(&self) -> &B {
        &self.backing
    }

    pub fn access(&self) -> DirectoryAccess {
        self.access
    }
}

impl DirectoryMapping {
    /// Hand the matching-host filesystem adapter a retained parent and one leaf.
    /// The adapter must enforce this mapping's access and avoid following the leaf.
    #[cfg(target_os = "linux")]
    pub fn bound_parent<'a>(&self, relative: &'a [u8]) -> io::Result<(std::fs::File, &'a str)> {
        self.backing.bound_parent(relative)
    }

    pub fn open_read(&self, relative: &[u8]) -> io::Result<std::fs::File> {
        self.backing.open_read(relative)
    }
    pub fn create_dir(&self, relative: &[u8]) -> io::Result<()> {
        self.require_write()?;
        self.backing.create_dir(relative)
    }

    pub fn open_write(&self, relative: &[u8]) -> io::Result<std::fs::File> {
        self.require_write()?;
        self.backing.open_write(relative)
    }

    fn require_write(&self) -> io::Result<()> {
        if self.access == DirectoryAccess::ReadOnly {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Ok(())
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
    fn a_read_only_subtree_preserves_nested_mounts_and_attenuates_every_entry() {
        let mut incoming = DirectoryMappings::default();
        incoming
            .insert("/", "source", DirectoryAccess::ReadWrite)
            .unwrap();
        incoming
            .insert("/sdk/headers", "headers", DirectoryAccess::ReadWrite)
            .unwrap();
        incoming
            .insert("/sdk/bin", "tools", DirectoryAccess::ReadOnly)
            .unwrap();
        incoming
            .insert("/sdk-next", "sibling", DirectoryAccess::ReadWrite)
            .unwrap();
        let child = incoming
            .remount("/sdk", "/", DirectoryAccess::ReadOnly, |root, relative| {
                assert_eq!(relative, b"sdk");
                Ok(*root)
            })
            .unwrap();
        assert_eq!(child.entries().count(), 3);
        for (path, label) in [
            ("/root", "source"),
            ("/headers/name", "headers"),
            ("/bin/tool", "tools"),
        ] {
            let (mapping, _) = child.resolve_file(path.as_bytes()).unwrap();
            assert_eq!(mapping.access(), DirectoryAccess::ReadOnly);
            assert_eq!(*mapping.backing(), label);
        }
        let same_root = child
            .remount("/", "/again", DirectoryAccess::ReadOnly, |root, tail| {
                assert!(tail.is_empty());
                Ok(*root)
            })
            .unwrap();
        assert_eq!(same_root.entries().count(), 3);
        assert_eq!(
            *same_root.resolve_entry(b"/again/bin").unwrap().0.backing(),
            "tools"
        );
    }

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
