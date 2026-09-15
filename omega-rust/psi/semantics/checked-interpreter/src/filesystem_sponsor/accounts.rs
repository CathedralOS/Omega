//! Filesystem accounts, their state, namespace entries and object records.

use crate::filesystem_sponsor::{
    FilesystemSponsorError, FilesystemSponsorLimits, FilesystemSponsorPath,
};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
pub(crate) struct FilesystemAccount {
    pub(crate) id: u64,
    pub(crate) session_root: PathBuf,
    pub(crate) limits: FilesystemSponsorLimits,
    pub(crate) committed: AccountState,
    pub(crate) prepared_transaction: Option<u64>,
    pub(crate) next_transaction_id: u64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AccountState {
    pub(crate) namespace: BTreeMap<PathBuf, NamespaceEntry>,
    pub(crate) objects: BTreeMap<ObjectId, ObjectRecord>,
    pub(crate) descriptors: BTreeMap<DescriptorId, ObjectId>,
    pub(crate) next_object_id: u64,
    pub(crate) next_descriptor_id: u64,
    pub(crate) entries: u64,
    pub(crate) total_logical_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NamespaceEntry {
    Directory,
    Symlink { spelling_bytes: u64 },
    Object(ObjectId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ObjectId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct DescriptorId(u64);

#[derive(Debug, Clone, Copy)]
pub(crate) struct ObjectRecord {
    pub(crate) extent: u64,
    pub(crate) names: u64,
    pub(crate) open_descriptors: u64,
}

impl AccountState {
    pub(crate) fn require_available_parent(
        &self,
        path: &Path,
    ) -> Result<(), FilesystemSponsorError> {
        if self.namespace.contains_key(path) {
            return Err(FilesystemSponsorError::EntryAlreadyExists(
                path.to_path_buf(),
            ));
        }
        let parent = path.parent().unwrap_or_else(|| Path::new(""));
        if parent.as_os_str().is_empty() {
            return Ok(());
        }
        match self.namespace.get(parent) {
            Some(NamespaceEntry::Directory) => Ok(()),
            Some(_) => Err(FilesystemSponsorError::ParentIsNotDirectory(
                parent.to_path_buf(),
            )),
            None => Err(FilesystemSponsorError::ParentEntryMissing(
                parent.to_path_buf(),
            )),
        }
    }

    pub(crate) fn insert_new(
        &mut self,
        path: &Path,
        entry: NamespaceEntry,
    ) -> Result<(), FilesystemSponsorError> {
        if self.namespace.insert(path.to_path_buf(), entry).is_some() {
            return Err(FilesystemSponsorError::EntryAlreadyExists(
                path.to_path_buf(),
            ));
        }
        Ok(())
    }

    pub(crate) fn allocate_object_id(&mut self) -> Result<ObjectId, FilesystemSponsorError> {
        let object_id = ObjectId(self.next_object_id);
        self.next_object_id = checked_add(self.next_object_id, 1)?;
        Ok(object_id)
    }

    fn allocate_descriptor_id(&mut self) -> Result<DescriptorId, FilesystemSponsorError> {
        let descriptor_id = DescriptorId(self.next_descriptor_id);
        self.next_descriptor_id = checked_add(self.next_descriptor_id, 1)?;
        Ok(descriptor_id)
    }

    pub(crate) fn open_object(
        &mut self,
        object_id: ObjectId,
    ) -> Result<DescriptorId, FilesystemSponsorError> {
        let descriptor_id = self.allocate_descriptor_id()?;
        let object = self
            .objects
            .get_mut(&object_id)
            .ok_or(FilesystemSponsorError::OpenDescriptorNotFound)?;
        object.open_descriptors = checked_add(object.open_descriptors, 1)?;
        self.descriptors.insert(descriptor_id, object_id);
        Ok(descriptor_id)
    }

    pub(crate) fn close_descriptor(
        &mut self,
        descriptor_id: DescriptorId,
    ) -> Result<(), FilesystemSponsorError> {
        let object_id = self
            .descriptors
            .remove(&descriptor_id)
            .ok_or(FilesystemSponsorError::OpenDescriptorNotFound)?;
        let object = self
            .objects
            .get_mut(&object_id)
            .ok_or(FilesystemSponsorError::TransactionNoLongerCurrent)?;
        object.open_descriptors = checked_sub(object.open_descriptors, 1)?;
        self.remove_dead_object(object_id);
        Ok(())
    }

    pub(crate) fn set_descriptor_extent(
        &mut self,
        descriptor_id: DescriptorId,
        extent: u64,
    ) -> Result<(), FilesystemSponsorError> {
        let object_id = *self
            .descriptors
            .get(&descriptor_id)
            .ok_or(FilesystemSponsorError::OpenDescriptorNotFound)?;
        self.objects
            .get_mut(&object_id)
            .ok_or(FilesystemSponsorError::TransactionNoLongerCurrent)?
            .extent = extent;
        Ok(())
    }

    pub(crate) fn extend_object(
        &mut self,
        object_id: ObjectId,
        offset: u64,
        bytes: u64,
    ) -> Result<(), FilesystemSponsorError> {
        if bytes == 0 {
            return self
                .objects
                .contains_key(&object_id)
                .then_some(())
                .ok_or(FilesystemSponsorError::OpenDescriptorNotFound);
        }
        let end = checked_add(offset, bytes)?;
        let object = self
            .objects
            .get_mut(&object_id)
            .ok_or(FilesystemSponsorError::OpenDescriptorNotFound)?;
        object.extent = object.extent.max(end);
        Ok(())
    }

    pub(crate) fn unlink(&mut self, path: &Path) -> Result<(), FilesystemSponsorError> {
        let entry = *self
            .namespace
            .get(path)
            .ok_or_else(|| FilesystemSponsorError::EntryNotFound(path.to_path_buf()))?;
        if entry == NamespaceEntry::Directory && self.has_descendants(path) {
            return Err(FilesystemSponsorError::DirectoryNotEmpty(
                path.to_path_buf(),
            ));
        }
        self.remove_namespace_entry(path)?;
        Ok(())
    }

    pub(crate) fn rename(
        &mut self,
        source: &Path,
        destination: &Path,
    ) -> Result<(), FilesystemSponsorError> {
        let source_entry = *self
            .namespace
            .get(source)
            .ok_or_else(|| FilesystemSponsorError::EntryNotFound(source.to_path_buf()))?;
        if source == destination {
            return Ok(());
        }
        if source_entry == NamespaceEntry::Directory && destination.starts_with(source) {
            return Err(FilesystemSponsorError::InvalidDirectoryRename(
                destination.to_path_buf(),
            ));
        }
        self.require_destination_parent(destination)?;

        if let Some(destination_entry) = self.namespace.get(destination).copied() {
            if matches!(source_entry, NamespaceEntry::Object(source_id)
                if destination_entry == NamespaceEntry::Object(source_id))
            {
                return Ok(());
            }
            let source_is_directory = source_entry == NamespaceEntry::Directory;
            let destination_is_directory = destination_entry == NamespaceEntry::Directory;
            if source_is_directory != destination_is_directory {
                return Err(FilesystemSponsorError::EntryAlreadyExists(
                    destination.to_path_buf(),
                ));
            }
            if destination_is_directory && self.has_descendants(destination) {
                return Err(FilesystemSponsorError::DirectoryNotEmpty(
                    destination.to_path_buf(),
                ));
            }
            self.remove_namespace_entry(destination)?;
        }

        let moved_paths: Vec<_> = self
            .namespace
            .keys()
            .filter(|path| path.as_path() == source || path.starts_with(source))
            .cloned()
            .collect();
        for old_path in moved_paths {
            let suffix = old_path
                .strip_prefix(source)
                .map_err(|_| FilesystemSponsorError::TransactionNoLongerCurrent)?;
            let new_path = destination.join(suffix);
            let entry = self
                .namespace
                .remove(&old_path)
                .ok_or(FilesystemSponsorError::TransactionNoLongerCurrent)?;
            if self.namespace.insert(new_path.clone(), entry).is_some() {
                return Err(FilesystemSponsorError::EntryAlreadyExists(new_path));
            }
        }
        Ok(())
    }

    fn require_destination_parent(&self, path: &Path) -> Result<(), FilesystemSponsorError> {
        let parent = path.parent().unwrap_or_else(|| Path::new(""));
        if parent.as_os_str().is_empty() {
            return Ok(());
        }
        match self.namespace.get(parent) {
            Some(NamespaceEntry::Directory) => Ok(()),
            Some(_) => Err(FilesystemSponsorError::ParentIsNotDirectory(
                parent.to_path_buf(),
            )),
            None => Err(FilesystemSponsorError::ParentEntryMissing(
                parent.to_path_buf(),
            )),
        }
    }

    fn has_descendants(&self, path: &Path) -> bool {
        self.namespace
            .keys()
            .any(|candidate| candidate != path && candidate.starts_with(path))
    }

    fn remove_namespace_entry(
        &mut self,
        path: &Path,
    ) -> Result<NamespaceEntry, FilesystemSponsorError> {
        let entry = self
            .namespace
            .remove(path)
            .ok_or_else(|| FilesystemSponsorError::EntryNotFound(path.to_path_buf()))?;
        if let NamespaceEntry::Object(object_id) = entry {
            let object = self
                .objects
                .get_mut(&object_id)
                .ok_or(FilesystemSponsorError::TransactionNoLongerCurrent)?;
            object.names = checked_sub(object.names, 1)?;
            self.remove_dead_object(object_id);
        }
        Ok(entry)
    }

    fn remove_dead_object(&mut self, object_id: ObjectId) {
        if self
            .objects
            .get(&object_id)
            .is_some_and(|object| object.names == 0 && object.open_descriptors == 0)
        {
            self.objects.remove(&object_id);
        }
    }

    pub(crate) fn recalculate_and_validate(
        &mut self,
        limits: FilesystemSponsorLimits,
    ) -> Result<(), FilesystemSponsorError> {
        let entries = usize_to_u64(self.namespace.len())?;
        if entries > limits.maximum_entries {
            return Err(FilesystemSponsorError::EntryLimitExceeded {
                limit: limits.maximum_entries,
                attempted: entries,
            });
        }

        let mut names = BTreeMap::<ObjectId, u64>::new();
        let mut total_logical_bytes = 0_u64;
        for entry in self.namespace.values() {
            match entry {
                NamespaceEntry::Directory => {}
                NamespaceEntry::Symlink { spelling_bytes } => {
                    total_logical_bytes = checked_add(total_logical_bytes, *spelling_bytes)?;
                }
                NamespaceEntry::Object(object_id) => {
                    let count = names.entry(*object_id).or_default();
                    *count = checked_add(*count, 1)?;
                }
            }
        }

        let mut opens = BTreeMap::<ObjectId, u64>::new();
        for object_id in self.descriptors.values() {
            let count = opens.entry(*object_id).or_default();
            *count = checked_add(*count, 1)?;
        }

        for (object_id, object) in &self.objects {
            if object.extent > limits.maximum_object_extent {
                return Err(FilesystemSponsorError::ObjectExtentLimitExceeded {
                    limit: limits.maximum_object_extent,
                    attempted: object.extent,
                });
            }
            if names.get(object_id).copied().unwrap_or(0) != object.names
                || opens.get(object_id).copied().unwrap_or(0) != object.open_descriptors
                || (object.names == 0 && object.open_descriptors == 0)
            {
                return Err(FilesystemSponsorError::TransactionNoLongerCurrent);
            }
            total_logical_bytes = checked_add(total_logical_bytes, object.extent)?;
        }
        if names
            .keys()
            .any(|object_id| !self.objects.contains_key(object_id))
            || opens
                .keys()
                .any(|object_id| !self.objects.contains_key(object_id))
        {
            return Err(FilesystemSponsorError::TransactionNoLongerCurrent);
        }
        if total_logical_bytes > limits.maximum_total_logical_bytes {
            return Err(FilesystemSponsorError::TotalLogicalBytesLimitExceeded {
                limit: limits.maximum_total_logical_bytes,
                attempted: total_logical_bytes,
            });
        }
        self.entries = entries;
        self.total_logical_bytes = total_logical_bytes;
        Ok(())
    }
}

pub(crate) fn check_account_path(
    account_id: u64,
    path: &FilesystemSponsorPath,
) -> Result<(), FilesystemSponsorError> {
    if path.account_id != account_id {
        return Err(FilesystemSponsorError::CrossAccountOperation);
    }
    Ok(())
}

pub(crate) fn normalize_absolute(path: &Path) -> Result<PathBuf, FilesystemSponsorError> {
    if !path.is_absolute() {
        return Err(FilesystemSponsorError::PathMustBeAbsolute(
            path.to_path_buf(),
        ));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(FilesystemSponsorError::PathEscapesFilesystemRoot(
                        path.to_path_buf(),
                    ));
                }
            }
            Component::Normal(name) => normalized.push(name),
        }
    }
    Ok(normalized)
}

pub(crate) fn checked_add(left: u64, right: u64) -> Result<u64, FilesystemSponsorError> {
    left.checked_add(right)
        .ok_or(FilesystemSponsorError::ArithmeticOverflow)
}

fn checked_sub(left: u64, right: u64) -> Result<u64, FilesystemSponsorError> {
    left.checked_sub(right)
        .ok_or(FilesystemSponsorError::ArithmeticOverflow)
}

pub(crate) fn usize_to_u64(value: usize) -> Result<u64, FilesystemSponsorError> {
    u64::try_from(value).map_err(|_| FilesystemSponsorError::ArithmeticOverflow)
}
