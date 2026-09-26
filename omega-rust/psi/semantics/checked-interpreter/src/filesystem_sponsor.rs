#![forbid(unsafe_code)]

//! Compiler-owned accounting for a disposable build-filesystem session.
//!
//! Accounting remains independent of either filesystem provider. A
//! provider prepares an accounting transaction before attempting its mutation,
//! commits the token only after the provider succeeds, and otherwise drops or
//! aborts the token. One outstanding token reserves the account, so committed
//! state cannot change between provider preflight and accounting commit.
//!
//! This file owns the sponsor, its limits and its account operations.
//! `private_staging.rs` owns optional fresh-directory custody and cleanup;
//! Omega still decides which grants and acceptance classification are lawful.
//! `sponsor_errors.rs` carries the sponsor error, `snapshots.rs` sponsor
//! paths, descriptors and namespace snapshots,
//! `prepared_transactions.rs` prepared mutations, opens, writes and
//! account transactions, `accounts.rs` accounts, their state and object
//! records and `tests.rs` the sponsor tests.

mod accounts;
mod prepared_transactions;
mod private_staging;
mod snapshots;
mod sponsor_errors;
#[cfg(test)]
mod tests;

pub use prepared_transactions::{
    PreparedFilesystemMutation, PreparedFilesystemOpen, PreparedFilesystemWrite,
};
pub use snapshots::{
    FilesystemOpenDescriptor, FilesystemSponsorEntry, FilesystemSponsorNamespaceEntry,
    FilesystemSponsorNamespaceEntryKind, FilesystemSponsorNamespaceSnapshot, FilesystemSponsorPath,
    FilesystemSponsorSnapshot,
};
pub use sponsor_errors::FilesystemSponsorError;

use crate::filesystem_sponsor::accounts::{
    AccountState, DescriptorId, FilesystemAccount, NamespaceEntry, ObjectRecord,
    check_account_path, checked_add, normalize_absolute, usize_to_u64,
};
use crate::filesystem_sponsor::prepared_transactions::PreparedAccountTransaction;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

pub const COMPILER_DEFAULT_STAGING_ENTRY_LIMIT: u64 = 4_096;

pub const COMPILER_DEFAULT_STAGING_TOTAL_LOGICAL_BYTES: u64 = 256 * 1024 * 1024;

pub const COMPILER_DEFAULT_STAGING_MAX_OBJECT_EXTENT: u64 = 256 * 1024 * 1024;

static NEXT_ACCOUNT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemSponsorLimits {
    pub maximum_entries: u64,
    pub maximum_total_logical_bytes: u64,
    pub maximum_object_extent: u64,
}

impl FilesystemSponsorLimits {
    pub const COMPILER_DEFAULT: Self = Self {
        maximum_entries: COMPILER_DEFAULT_STAGING_ENTRY_LIMIT,
        maximum_total_logical_bytes: COMPILER_DEFAULT_STAGING_TOTAL_LOGICAL_BYTES,
        maximum_object_extent: COMPILER_DEFAULT_STAGING_MAX_OBJECT_EXTENT,
    };
}

impl Default for FilesystemSponsorLimits {
    fn default() -> Self {
        Self::COMPILER_DEFAULT
    }
}

#[derive(Debug, Clone)]
pub struct FilesystemSponsor {
    account: Arc<Mutex<FilesystemAccount>>,
    private_staging: Option<Arc<private_staging::PrivateStaging>>,
}

impl PartialEq for FilesystemSponsor {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.account, &other.account)
    }
}

impl Eq for FilesystemSponsor {}

impl FilesystemSponsor {
    pub fn new(session_root: impl AsRef<Path>) -> Result<Self, FilesystemSponsorError> {
        Self::with_limits(session_root, FilesystemSponsorLimits::COMPILER_DEFAULT)
    }

    pub fn with_limits(
        session_root: impl AsRef<Path>,
        limits: FilesystemSponsorLimits,
    ) -> Result<Self, FilesystemSponsorError> {
        let session_root = normalize_absolute(session_root.as_ref())?;
        let id = NEXT_ACCOUNT_ID
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| FilesystemSponsorError::AccountIdentityExhausted)?;
        Ok(Self {
            private_staging: None,
            account: Arc::new(Mutex::new(FilesystemAccount {
                id,
                session_root,
                limits,
                committed: AccountState {
                    next_object_id: 1,
                    next_descriptor_id: 1,
                    ..AccountState::default()
                },
                prepared_transaction: None,
                next_transaction_id: 1,
            })),
        })
    }

    pub fn limits(&self) -> Result<FilesystemSponsorLimits, FilesystemSponsorError> {
        Ok(self.lock_account()?.limits)
    }

    pub fn session_root(&self) -> Result<PathBuf, FilesystemSponsorError> {
        Ok(self.lock_account()?.session_root.clone())
    }

    /// Bind an absolute provider path to this account after lexical containment
    /// checking. The account namespace never follows symlinks while resolving a
    /// bound path.
    pub fn bind_path(
        &self,
        absolute_path: impl AsRef<Path>,
    ) -> Result<FilesystemSponsorPath, FilesystemSponsorError> {
        let normalized = normalize_absolute(absolute_path.as_ref())?;
        let account = self.lock_account()?;
        let relative = normalized
            .strip_prefix(&account.session_root)
            .map_err(|_| FilesystemSponsorError::PathOutsideSessionRoot(normalized.clone()))?;
        if relative.as_os_str().is_empty() {
            return Err(FilesystemSponsorError::SessionRootIsNotAnEntry);
        }
        Ok(FilesystemSponsorPath {
            account_id: account.id,
            relative: relative.to_path_buf(),
        })
    }

    pub fn snapshot(&self) -> Result<FilesystemSponsorSnapshot, FilesystemSponsorError> {
        let account = self.lock_account()?;
        Ok(FilesystemSponsorSnapshot {
            entries: account.committed.entries,
            total_logical_bytes: account.committed.total_logical_bytes,
            unique_objects: usize_to_u64(account.committed.objects.len())?,
            open_descriptors: usize_to_u64(account.committed.descriptors.len())?,
        })
    }

    /// Read-only logical namespace and quiescence evidence for compiler-owned
    /// staged-output capture. Object groups are account-local correlation IDs;
    /// they are never canonical package identity.
    pub fn namespace_snapshot(
        &self,
    ) -> Result<FilesystemSponsorNamespaceSnapshot, FilesystemSponsorError> {
        let account = self.lock_account()?;
        let entries = account
            .committed
            .namespace
            .iter()
            .map(|(relative_path, entry)| {
                let kind = match entry {
                    NamespaceEntry::Directory => FilesystemSponsorNamespaceEntryKind::Directory,
                    NamespaceEntry::Symlink { spelling_bytes } => {
                        FilesystemSponsorNamespaceEntryKind::Symlink {
                            spelling_bytes: *spelling_bytes,
                        }
                    }
                    NamespaceEntry::Object(object_id) => {
                        let object = account
                            .committed
                            .objects
                            .get(object_id)
                            .ok_or(FilesystemSponsorError::TransactionNoLongerCurrent)?;
                        FilesystemSponsorNamespaceEntryKind::Object {
                            group: object_id.0,
                            extent: object.extent,
                        }
                    }
                };
                Ok(FilesystemSponsorNamespaceEntry {
                    relative_path: relative_path.clone(),
                    kind,
                })
            })
            .collect::<Result<Vec<_>, FilesystemSponsorError>>()?;
        Ok(FilesystemSponsorNamespaceSnapshot {
            entries,
            open_descriptors: usize_to_u64(account.committed.descriptors.len())?,
            transaction_prepared: account.prepared_transaction.is_some(),
        })
    }

    pub fn entry(
        &self,
        path: &FilesystemSponsorPath,
    ) -> Result<Option<FilesystemSponsorEntry>, FilesystemSponsorError> {
        let account = self.lock_account()?;
        check_account_path(account.id, path)?;
        let Some(entry) = account.committed.namespace.get(&path.relative) else {
            return Ok(None);
        };
        let entry = match entry {
            NamespaceEntry::Directory => FilesystemSponsorEntry::Directory,
            NamespaceEntry::Symlink { spelling_bytes } => FilesystemSponsorEntry::Symlink {
                spelling_bytes: *spelling_bytes,
            },
            NamespaceEntry::Object(object_id) => {
                let object = account
                    .committed
                    .objects
                    .get(object_id)
                    .ok_or(FilesystemSponsorError::TransactionNoLongerCurrent)?;
                FilesystemSponsorEntry::Object {
                    extent: object.extent,
                    names: object.names,
                    open_descriptors: object.open_descriptors,
                }
            }
        };
        Ok(Some(entry))
    }

    pub fn prepare_create_directory(
        &self,
        path: &FilesystemSponsorPath,
    ) -> Result<PreparedFilesystemMutation, FilesystemSponsorError> {
        self.prepare_fixed(&[path], |state, _, path| {
            state.require_available_parent(path)?;
            state.insert_new(path, NamespaceEntry::Directory)
        })
    }

    pub fn prepare_create_object(
        &self,
        path: &FilesystemSponsorPath,
        initial_extent: u64,
    ) -> Result<PreparedFilesystemMutation, FilesystemSponsorError> {
        self.prepare_fixed(&[path], |state, _, path| {
            state.require_available_parent(path)?;
            let object_id = state.allocate_object_id()?;
            state.objects.insert(
                object_id,
                ObjectRecord {
                    extent: initial_extent,
                    names: 1,
                    open_descriptors: 0,
                },
            );
            state.insert_new(path, NamespaceEntry::Object(object_id))
        })
    }

    /// Prepare creation and the resulting open descriptor as one accounting
    /// transaction. A provider commits this token only after its create/open
    /// operation succeeds.
    pub fn prepare_create_object_open(
        &self,
        path: &FilesystemSponsorPath,
        initial_extent: u64,
    ) -> Result<PreparedFilesystemOpen, FilesystemSponsorError> {
        let (prepared, mut candidate, account_id) = self.begin_candidate(&[path])?;
        let result = (|| {
            candidate.require_available_parent(&path.relative)?;
            let object_id = candidate.allocate_object_id()?;
            candidate.objects.insert(
                object_id,
                ObjectRecord {
                    extent: initial_extent,
                    names: 1,
                    open_descriptors: 0,
                },
            );
            candidate.insert_new(&path.relative, NamespaceEntry::Object(object_id))?;
            candidate.open_object(object_id)
        })();
        self.finish_open(prepared, candidate, account_id, result)
    }

    pub fn prepare_create_symlink(
        &self,
        path: &FilesystemSponsorPath,
        target_spelling: &[u8],
    ) -> Result<PreparedFilesystemMutation, FilesystemSponsorError> {
        let spelling_bytes = usize_to_u64(target_spelling.len())?;
        self.prepare_fixed(&[path], move |state, _, path| {
            state.require_available_parent(path)?;
            state.insert_new(path, NamespaceEntry::Symlink { spelling_bytes })
        })
    }

    pub fn prepare_hard_link(
        &self,
        existing: &FilesystemSponsorPath,
        new_name: &FilesystemSponsorPath,
    ) -> Result<PreparedFilesystemMutation, FilesystemSponsorError> {
        self.prepare_fixed(&[existing, new_name], |state, existing, new_name| {
            state.require_available_parent(new_name)?;
            if state.namespace.contains_key(new_name) {
                return Err(FilesystemSponsorError::EntryAlreadyExists(new_name.clone()));
            }
            let object_id = match state.namespace.get(existing) {
                Some(NamespaceEntry::Object(object_id)) => *object_id,
                Some(_) => {
                    return Err(FilesystemSponsorError::EntryIsNotRegularObject(
                        existing.clone(),
                    ));
                }
                None => return Err(FilesystemSponsorError::EntryNotFound(existing.clone())),
            };
            let object = state
                .objects
                .get_mut(&object_id)
                .ok_or(FilesystemSponsorError::TransactionNoLongerCurrent)?;
            object.names = checked_add(object.names, 1)?;
            state
                .namespace
                .insert(new_name.clone(), NamespaceEntry::Object(object_id));
            Ok(())
        })
    }

    pub fn prepare_rename(
        &self,
        source: &FilesystemSponsorPath,
        destination: &FilesystemSponsorPath,
    ) -> Result<PreparedFilesystemMutation, FilesystemSponsorError> {
        self.prepare_fixed(&[source, destination], |state, source, destination| {
            state.rename(source, destination)
        })
    }

    pub fn prepare_unlink(
        &self,
        path: &FilesystemSponsorPath,
    ) -> Result<PreparedFilesystemMutation, FilesystemSponsorError> {
        self.prepare_fixed(&[path], |state, _, path| state.unlink(path))
    }

    pub fn prepare_open(
        &self,
        path: &FilesystemSponsorPath,
    ) -> Result<PreparedFilesystemOpen, FilesystemSponsorError> {
        self.prepare_open_with_extent(path, None)
    }

    /// Prepare an open of an existing object, optionally replacing its extent
    /// in the same transaction. `Some(0)` models `O_TRUNC`; `None` leaves the
    /// existing extent unchanged.
    pub fn prepare_open_with_extent(
        &self,
        path: &FilesystemSponsorPath,
        replacement_extent: Option<u64>,
    ) -> Result<PreparedFilesystemOpen, FilesystemSponsorError> {
        let (prepared, mut candidate, account_id) = self.begin_candidate(&[path])?;
        let object_id = match candidate.namespace.get(&path.relative) {
            Some(NamespaceEntry::Object(object_id)) => *object_id,
            Some(_) => {
                return Err(
                    prepared.cancel_with(FilesystemSponsorError::EntryIsNotRegularObject(
                        path.relative.clone(),
                    )),
                );
            }
            None => {
                return Err(prepared
                    .cancel_with(FilesystemSponsorError::EntryNotFound(path.relative.clone())));
            }
        };
        if let Some(extent) = replacement_extent {
            candidate
                .objects
                .get_mut(&object_id)
                .expect("namespace object identity was checked above")
                .extent = extent;
        }
        let result = candidate.open_object(object_id);
        self.finish_open(prepared, candidate, account_id, result)
    }

    /// Prepare a duplicate descriptor which refers to the same unique object
    /// and contributes one additional live-open count.
    pub fn prepare_duplicate(
        &self,
        descriptor: &FilesystemOpenDescriptor,
    ) -> Result<PreparedFilesystemOpen, FilesystemSponsorError> {
        self.check_descriptor_account(descriptor)?;
        let (prepared, mut candidate, account_id) = self.begin_candidate(&[])?;
        let result = candidate
            .descriptors
            .get(&descriptor.descriptor_id)
            .copied()
            .ok_or(FilesystemSponsorError::OpenDescriptorNotFound)
            .and_then(|object_id| candidate.open_object(object_id));
        self.finish_open(prepared, candidate, account_id, result)
    }

    pub fn prepare_close(
        &self,
        descriptor: &FilesystemOpenDescriptor,
    ) -> Result<PreparedFilesystemMutation, FilesystemSponsorError> {
        self.check_descriptor_account(descriptor)?;
        let (prepared, mut candidate, _) = self.begin_candidate(&[])?;
        let result = candidate.close_descriptor(descriptor.descriptor_id);
        self.finish_fixed(prepared, candidate, result)
    }

    pub fn prepare_set_extent(
        &self,
        descriptor: &FilesystemOpenDescriptor,
        new_extent: u64,
    ) -> Result<PreparedFilesystemMutation, FilesystemSponsorError> {
        self.check_descriptor_account(descriptor)?;
        let (prepared, mut candidate, _) = self.begin_candidate(&[])?;
        let result = candidate.set_descriptor_extent(descriptor.descriptor_id, new_extent);
        self.finish_fixed(prepared, candidate, result)
    }

    /// Reserve the worst-case extent for a provider write. After the provider
    /// succeeds, commit its actual byte count with
    /// [`PreparedFilesystemWrite::commit_written`].
    pub fn prepare_write(
        &self,
        descriptor: &FilesystemOpenDescriptor,
        offset: u64,
        requested_bytes: u64,
    ) -> Result<PreparedFilesystemWrite, FilesystemSponsorError> {
        self.check_descriptor_account(descriptor)?;
        let (prepared, base, _) = self.begin_candidate(&[])?;
        let limits = prepared.limits()?;
        let object_id = match base.descriptors.get(&descriptor.descriptor_id) {
            Some(object_id) => *object_id,
            None => {
                return Err(prepared.cancel_with(FilesystemSponsorError::OpenDescriptorNotFound));
            }
        };
        let mut worst_case = base.clone();
        let result = worst_case.extend_object(object_id, offset, requested_bytes);
        if let Err(error) = result.and_then(|()| worst_case.recalculate_and_validate(limits)) {
            return Err(prepared.cancel_with(error));
        }
        Ok(PreparedFilesystemWrite {
            prepared,
            base: Some(base),
            object_id,
            offset,
            prepared_bytes: requested_bytes,
            limits,
        })
    }

    fn prepare_fixed<F>(
        &self,
        paths: &[&FilesystemSponsorPath],
        mutate: F,
    ) -> Result<PreparedFilesystemMutation, FilesystemSponsorError>
    where
        F: FnOnce(&mut AccountState, &PathBuf, &PathBuf) -> Result<(), FilesystemSponsorError>,
    {
        let (prepared, mut candidate, _) = self.begin_candidate(paths)?;
        let first = paths
            .first()
            .map_or_else(PathBuf::new, |path| path.relative.clone());
        let last = paths
            .last()
            .map_or_else(PathBuf::new, |path| path.relative.clone());
        let result = mutate(&mut candidate, &first, &last);
        self.finish_fixed(prepared, candidate, result)
    }

    fn finish_fixed(
        &self,
        prepared: PreparedAccountTransaction,
        mut candidate: AccountState,
        result: Result<(), FilesystemSponsorError>,
    ) -> Result<PreparedFilesystemMutation, FilesystemSponsorError> {
        if let Err(error) = result {
            return Err(prepared.cancel_with(error));
        }
        let limits = prepared.limits()?;
        if let Err(error) = candidate.recalculate_and_validate(limits) {
            return Err(prepared.cancel_with(error));
        }
        Ok(PreparedFilesystemMutation {
            prepared,
            candidate: Some(candidate),
        })
    }

    fn finish_open(
        &self,
        prepared: PreparedAccountTransaction,
        mut candidate: AccountState,
        account_id: u64,
        result: Result<DescriptorId, FilesystemSponsorError>,
    ) -> Result<PreparedFilesystemOpen, FilesystemSponsorError> {
        let descriptor_id = match result {
            Ok(descriptor_id) => descriptor_id,
            Err(error) => return Err(prepared.cancel_with(error)),
        };
        let limits = prepared.limits()?;
        if let Err(error) = candidate.recalculate_and_validate(limits) {
            return Err(prepared.cancel_with(error));
        }
        Ok(PreparedFilesystemOpen {
            prepared,
            candidate: Some(candidate),
            descriptor: FilesystemOpenDescriptor {
                account_id,
                descriptor_id,
            },
        })
    }

    fn begin_candidate(
        &self,
        paths: &[&FilesystemSponsorPath],
    ) -> Result<(PreparedAccountTransaction, AccountState, u64), FilesystemSponsorError> {
        let mut account = self.lock_account()?;
        for path in paths {
            check_account_path(account.id, path)?;
        }
        if account.prepared_transaction.is_some() {
            return Err(FilesystemSponsorError::TransactionAlreadyPrepared);
        }
        let transaction_id = account.next_transaction_id;
        account.next_transaction_id = checked_add(account.next_transaction_id, 1)?;
        account.prepared_transaction = Some(transaction_id);
        Ok((
            PreparedAccountTransaction {
                account: Arc::clone(&self.account),
                transaction_id,
                active: true,
            },
            account.committed.clone(),
            account.id,
        ))
    }

    fn check_descriptor_account(
        &self,
        descriptor: &FilesystemOpenDescriptor,
    ) -> Result<(), FilesystemSponsorError> {
        if self.lock_account()?.id != descriptor.account_id {
            return Err(FilesystemSponsorError::CrossAccountOperation);
        }
        Ok(())
    }

    fn lock_account(&self) -> Result<MutexGuard<'_, FilesystemAccount>, FilesystemSponsorError> {
        self.account
            .lock()
            .map_err(|_| FilesystemSponsorError::AccountPoisoned)
    }
}
