#![forbid(unsafe_code)]

//! Compiler-owned accounting for a disposable build-filesystem session.
//!
//! This module deliberately knows nothing about either filesystem provider. A
//! provider prepares an accounting transaction before attempting its mutation,
//! commits the token only after the provider succeeds, and otherwise drops or
//! aborts the token. One outstanding token reserves the account, so committed
//! state cannot change between provider preflight and accounting commit.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::path::{Component, Path, PathBuf};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesystemSponsorError {
    PathMustBeAbsolute(PathBuf),
    PathEscapesFilesystemRoot(PathBuf),
    PathOutsideSessionRoot(PathBuf),
    SessionRootIsNotAnEntry,
    CrossAccountOperation,
    ParentEntryMissing(PathBuf),
    ParentIsNotDirectory(PathBuf),
    EntryAlreadyExists(PathBuf),
    EntryNotFound(PathBuf),
    EntryIsNotRegularObject(PathBuf),
    DirectoryNotEmpty(PathBuf),
    InvalidDirectoryRename(PathBuf),
    OpenDescriptorNotFound,
    TransactionAlreadyPrepared,
    TransactionNoLongerCurrent,
    EntryLimitExceeded { limit: u64, attempted: u64 },
    TotalLogicalBytesLimitExceeded { limit: u64, attempted: u64 },
    ObjectExtentLimitExceeded { limit: u64, attempted: u64 },
    PartialWriteExceedsPrepared { prepared: u64, actual: u64 },
    ArithmeticOverflow,
    AccountIdentityExhausted,
    AccountPoisoned,
}

impl fmt::Display for FilesystemSponsorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PathMustBeAbsolute(path) => {
                write!(
                    formatter,
                    "filesystem sponsor path is not absolute: {}",
                    path.display()
                )
            }
            Self::PathEscapesFilesystemRoot(path) => write!(
                formatter,
                "filesystem sponsor path escapes the filesystem root: {}",
                path.display()
            ),
            Self::PathOutsideSessionRoot(path) => write!(
                formatter,
                "filesystem sponsor path is outside its session root: {}",
                path.display()
            ),
            Self::SessionRootIsNotAnEntry => {
                formatter.write_str("the session root is excluded from sponsored entries")
            }
            Self::CrossAccountOperation => {
                formatter.write_str("filesystem operation crosses sponsor accounts")
            }
            Self::ParentEntryMissing(path) => {
                write!(formatter, "parent entry does not exist: {}", path.display())
            }
            Self::ParentIsNotDirectory(path) => {
                write!(
                    formatter,
                    "parent entry is not a directory: {}",
                    path.display()
                )
            }
            Self::EntryAlreadyExists(path) => {
                write!(formatter, "entry already exists: {}", path.display())
            }
            Self::EntryNotFound(path) => {
                write!(formatter, "entry does not exist: {}", path.display())
            }
            Self::EntryIsNotRegularObject(path) => {
                write!(
                    formatter,
                    "entry is not a regular object: {}",
                    path.display()
                )
            }
            Self::DirectoryNotEmpty(path) => {
                write!(formatter, "directory is not empty: {}", path.display())
            }
            Self::InvalidDirectoryRename(path) => write!(
                formatter,
                "directory cannot be renamed beneath itself: {}",
                path.display()
            ),
            Self::OpenDescriptorNotFound => formatter.write_str("open descriptor does not exist"),
            Self::TransactionAlreadyPrepared => {
                formatter.write_str("another filesystem accounting transaction is prepared")
            }
            Self::TransactionNoLongerCurrent => {
                formatter.write_str("filesystem accounting transaction is no longer current")
            }
            Self::EntryLimitExceeded { limit, attempted } => write!(
                formatter,
                "filesystem entry limit {limit} would be exceeded by {attempted} entries"
            ),
            Self::TotalLogicalBytesLimitExceeded { limit, attempted } => write!(
                formatter,
                "filesystem logical-byte limit {limit} would be exceeded by {attempted} bytes"
            ),
            Self::ObjectExtentLimitExceeded { limit, attempted } => write!(
                formatter,
                "filesystem object-extent limit {limit} would be exceeded by {attempted} bytes"
            ),
            Self::PartialWriteExceedsPrepared { prepared, actual } => write!(
                formatter,
                "provider reported {actual} written bytes after preparing at most {prepared}"
            ),
            Self::ArithmeticOverflow => {
                formatter.write_str("filesystem accounting arithmetic overflowed")
            }
            Self::AccountIdentityExhausted => {
                formatter.write_str("filesystem sponsor account identity space is exhausted")
            }
            Self::AccountPoisoned => formatter.write_str("filesystem sponsor account is poisoned"),
        }
    }
}

impl Error for FilesystemSponsorError {}

impl FilesystemSponsorError {
    pub const fn is_limit_exceeded(&self) -> bool {
        matches!(
            self,
            Self::EntryLimitExceeded { .. }
                | Self::TotalLogicalBytesLimitExceeded { .. }
                | Self::ObjectExtentLimitExceeded { .. }
                | Self::ArithmeticOverflow
        )
    }
}

#[derive(Debug, Clone)]
pub struct FilesystemSponsor {
    account: Arc<Mutex<FilesystemAccount>>,
}

impl PartialEq for FilesystemSponsor {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.account, &other.account)
    }
}

impl Eq for FilesystemSponsor {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSponsorPath {
    account_id: u64,
    relative: PathBuf,
}

impl FilesystemSponsorPath {
    pub fn relative(&self) -> &Path {
        &self.relative
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemOpenDescriptor {
    account_id: u64,
    descriptor_id: DescriptorId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemSponsorSnapshot {
    pub entries: u64,
    pub total_logical_bytes: u64,
    pub unique_objects: u64,
    pub open_descriptors: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSponsorNamespaceSnapshot {
    entries: Vec<FilesystemSponsorNamespaceEntry>,
    open_descriptors: u64,
    transaction_prepared: bool,
}

impl FilesystemSponsorNamespaceSnapshot {
    pub fn entries(&self) -> &[FilesystemSponsorNamespaceEntry] {
        &self.entries
    }

    pub const fn open_descriptors(&self) -> u64 {
        self.open_descriptors
    }

    pub const fn transaction_prepared(&self) -> bool {
        self.transaction_prepared
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSponsorNamespaceEntry {
    relative_path: PathBuf,
    kind: FilesystemSponsorNamespaceEntryKind,
}

impl FilesystemSponsorNamespaceEntry {
    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    pub const fn kind(&self) -> FilesystemSponsorNamespaceEntryKind {
        self.kind
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemSponsorNamespaceEntryKind {
    Directory,
    Symlink { spelling_bytes: u64 },
    Object { group: u64, extent: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemSponsorEntry {
    Directory,
    Symlink {
        spelling_bytes: u64,
    },
    Object {
        extent: u64,
        names: u64,
        open_descriptors: u64,
    },
}

#[derive(Debug)]
pub struct PreparedFilesystemMutation {
    prepared: PreparedAccountTransaction,
    candidate: Option<AccountState>,
}

#[derive(Debug)]
pub struct PreparedFilesystemOpen {
    prepared: PreparedAccountTransaction,
    candidate: Option<AccountState>,
    descriptor: FilesystemOpenDescriptor,
}

#[derive(Debug)]
pub struct PreparedFilesystemWrite {
    prepared: PreparedAccountTransaction,
    base: Option<AccountState>,
    object_id: ObjectId,
    offset: u64,
    prepared_bytes: u64,
    limits: FilesystemSponsorLimits,
}

#[derive(Debug)]
struct PreparedAccountTransaction {
    account: Arc<Mutex<FilesystemAccount>>,
    transaction_id: u64,
    active: bool,
}

#[derive(Debug)]
struct FilesystemAccount {
    id: u64,
    session_root: PathBuf,
    limits: FilesystemSponsorLimits,
    committed: AccountState,
    prepared_transaction: Option<u64>,
    next_transaction_id: u64,
}

#[derive(Debug, Clone, Default)]
struct AccountState {
    namespace: BTreeMap<PathBuf, NamespaceEntry>,
    objects: BTreeMap<ObjectId, ObjectRecord>,
    descriptors: BTreeMap<DescriptorId, ObjectId>,
    next_object_id: u64,
    next_descriptor_id: u64,
    entries: u64,
    total_logical_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NamespaceEntry {
    Directory,
    Symlink { spelling_bytes: u64 },
    Object(ObjectId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ObjectId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct DescriptorId(u64);

#[derive(Debug, Clone, Copy)]
struct ObjectRecord {
    extent: u64,
    names: u64,
    open_descriptors: u64,
}

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
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| FilesystemSponsorError::AccountIdentityExhausted)?;
        Ok(Self {
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

impl PreparedFilesystemMutation {
    pub fn commit(mut self) -> Result<(), FilesystemSponsorError> {
        let candidate = self
            .candidate
            .take()
            .ok_or(FilesystemSponsorError::TransactionNoLongerCurrent)?;
        self.prepared.commit(candidate)
    }

    pub fn abort(self) {}
}

impl PreparedFilesystemOpen {
    pub fn commit(mut self) -> Result<FilesystemOpenDescriptor, FilesystemSponsorError> {
        let candidate = self
            .candidate
            .take()
            .ok_or(FilesystemSponsorError::TransactionNoLongerCurrent)?;
        self.prepared.commit(candidate)?;
        Ok(self.descriptor)
    }

    pub fn abort(self) {}
}

impl PreparedFilesystemWrite {
    pub fn commit_written(mut self, actual_bytes: u64) -> Result<(), FilesystemSponsorError> {
        if actual_bytes > self.prepared_bytes {
            return Err(FilesystemSponsorError::PartialWriteExceedsPrepared {
                prepared: self.prepared_bytes,
                actual: actual_bytes,
            });
        }
        let mut candidate = self
            .base
            .take()
            .ok_or(FilesystemSponsorError::TransactionNoLongerCurrent)?;
        candidate.extend_object(self.object_id, self.offset, actual_bytes)?;
        candidate.recalculate_and_validate(self.limits)?;
        self.prepared.commit(candidate)
    }

    pub fn abort(self) {}
}

impl PreparedAccountTransaction {
    fn limits(&self) -> Result<FilesystemSponsorLimits, FilesystemSponsorError> {
        Ok(self
            .account
            .lock()
            .map_err(|_| FilesystemSponsorError::AccountPoisoned)?
            .limits)
    }

    fn commit(mut self, candidate: AccountState) -> Result<(), FilesystemSponsorError> {
        {
            let mut account = self
                .account
                .lock()
                .map_err(|_| FilesystemSponsorError::AccountPoisoned)?;
            if account.prepared_transaction != Some(self.transaction_id) {
                return Err(FilesystemSponsorError::TransactionNoLongerCurrent);
            }
            account.committed = candidate;
            account.prepared_transaction = None;
        }
        self.active = false;
        Ok(())
    }

    fn cancel_with(mut self, error: FilesystemSponsorError) -> FilesystemSponsorError {
        self.cancel();
        error
    }

    fn cancel(&mut self) {
        if !self.active {
            return;
        }
        if let Ok(mut account) = self.account.lock()
            && account.prepared_transaction == Some(self.transaction_id)
        {
            account.prepared_transaction = None;
        }
        self.active = false;
    }
}

impl Drop for PreparedAccountTransaction {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl AccountState {
    fn require_available_parent(&self, path: &Path) -> Result<(), FilesystemSponsorError> {
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

    fn insert_new(
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

    fn allocate_object_id(&mut self) -> Result<ObjectId, FilesystemSponsorError> {
        let object_id = ObjectId(self.next_object_id);
        self.next_object_id = checked_add(self.next_object_id, 1)?;
        Ok(object_id)
    }

    fn allocate_descriptor_id(&mut self) -> Result<DescriptorId, FilesystemSponsorError> {
        let descriptor_id = DescriptorId(self.next_descriptor_id);
        self.next_descriptor_id = checked_add(self.next_descriptor_id, 1)?;
        Ok(descriptor_id)
    }

    fn open_object(&mut self, object_id: ObjectId) -> Result<DescriptorId, FilesystemSponsorError> {
        let descriptor_id = self.allocate_descriptor_id()?;
        let object = self
            .objects
            .get_mut(&object_id)
            .ok_or(FilesystemSponsorError::OpenDescriptorNotFound)?;
        object.open_descriptors = checked_add(object.open_descriptors, 1)?;
        self.descriptors.insert(descriptor_id, object_id);
        Ok(descriptor_id)
    }

    fn close_descriptor(
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

    fn set_descriptor_extent(
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

    fn extend_object(
        &mut self,
        object_id: ObjectId,
        offset: u64,
        bytes: u64,
    ) -> Result<(), FilesystemSponsorError> {
        let end = checked_add(offset, bytes)?;
        let object = self
            .objects
            .get_mut(&object_id)
            .ok_or(FilesystemSponsorError::OpenDescriptorNotFound)?;
        object.extent = object.extent.max(end);
        Ok(())
    }

    fn unlink(&mut self, path: &Path) -> Result<(), FilesystemSponsorError> {
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

    fn rename(&mut self, source: &Path, destination: &Path) -> Result<(), FilesystemSponsorError> {
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

    fn recalculate_and_validate(
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

fn check_account_path(
    account_id: u64,
    path: &FilesystemSponsorPath,
) -> Result<(), FilesystemSponsorError> {
    if path.account_id != account_id {
        return Err(FilesystemSponsorError::CrossAccountOperation);
    }
    Ok(())
}

fn normalize_absolute(path: &Path) -> Result<PathBuf, FilesystemSponsorError> {
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

fn checked_add(left: u64, right: u64) -> Result<u64, FilesystemSponsorError> {
    left.checked_add(right)
        .ok_or(FilesystemSponsorError::ArithmeticOverflow)
}

fn checked_sub(left: u64, right: u64) -> Result<u64, FilesystemSponsorError> {
    left.checked_sub(right)
        .ok_or(FilesystemSponsorError::ArithmeticOverflow)
}

fn usize_to_u64(value: usize) -> Result<u64, FilesystemSponsorError> {
    u64::try_from(value).map_err(|_| FilesystemSponsorError::ArithmeticOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits(entries: u64, total: u64, extent: u64) -> FilesystemSponsorLimits {
        FilesystemSponsorLimits {
            maximum_entries: entries,
            maximum_total_logical_bytes: total,
            maximum_object_extent: extent,
        }
    }

    fn sponsor_with(limits: FilesystemSponsorLimits) -> FilesystemSponsor {
        FilesystemSponsor::with_limits("/staging/session", limits).unwrap()
    }

    fn path(sponsor: &FilesystemSponsor, suffix: &str) -> FilesystemSponsorPath {
        sponsor
            .bind_path(Path::new("/staging/session").join(suffix))
            .unwrap()
    }

    fn commit_directory(sponsor: &FilesystemSponsor, suffix: &str) {
        sponsor
            .prepare_create_directory(&path(sponsor, suffix))
            .unwrap()
            .commit()
            .unwrap();
    }

    fn commit_object(
        sponsor: &FilesystemSponsor,
        suffix: &str,
        extent: u64,
    ) -> FilesystemSponsorPath {
        let path = path(sponsor, suffix);
        sponsor
            .prepare_create_object(&path, extent)
            .unwrap()
            .commit()
            .unwrap();
        path
    }

    #[test]
    fn compiler_defaults_are_explicit_and_separate() {
        assert_eq!(FilesystemSponsorLimits::default().maximum_entries, 4_096);
        assert_eq!(
            FilesystemSponsorLimits::default().maximum_total_logical_bytes,
            256 * 1024 * 1024
        );
        assert_eq!(
            FilesystemSponsorLimits::default().maximum_object_extent,
            256 * 1024 * 1024
        );
    }

    #[test]
    fn session_root_is_excluded_and_outside_paths_are_rejected() {
        let sponsor = sponsor_with(limits(2, 2, 2));
        assert_eq!(
            sponsor.bind_path("/staging/session").unwrap_err(),
            FilesystemSponsorError::SessionRootIsNotAnEntry
        );
        assert!(matches!(
            sponsor.bind_path("/staging/elsewhere"),
            Err(FilesystemSponsorError::PathOutsideSessionRoot(_))
        ));
        assert!(matches!(
            sponsor.bind_path("relative"),
            Err(FilesystemSponsorError::PathMustBeAbsolute(_))
        ));
        assert_eq!(sponsor.snapshot().unwrap().entries, 0);
    }

    #[test]
    fn entry_total_and_extent_limits_fail_during_prepare_without_committing() {
        let sponsor = sponsor_with(limits(2, 7, 5));
        commit_object(&sponsor, "first", 5);
        sponsor
            .prepare_create_symlink(&path(&sponsor, "link"), b"xy")
            .unwrap()
            .commit()
            .unwrap();

        assert!(matches!(
            sponsor.prepare_create_directory(&path(&sponsor, "third")),
            Err(FilesystemSponsorError::EntryLimitExceeded {
                limit: 2,
                attempted: 3
            })
        ));
        sponsor
            .prepare_unlink(&path(&sponsor, "link"))
            .unwrap()
            .commit()
            .unwrap();
        assert!(matches!(
            sponsor.prepare_create_object(&path(&sponsor, "too-large"), 6),
            Err(FilesystemSponsorError::ObjectExtentLimitExceeded {
                limit: 5,
                attempted: 6
            })
        ));

        assert!(matches!(
            sponsor.prepare_create_symlink(&path(&sponsor, "bytes"), b"xyz"),
            Err(FilesystemSponsorError::TotalLogicalBytesLimitExceeded {
                limit: 7,
                attempted: 8
            })
        ));
        assert_eq!(
            sponsor.snapshot().unwrap(),
            FilesystemSponsorSnapshot {
                entries: 1,
                total_logical_bytes: 5,
                unique_objects: 1,
                open_descriptors: 0,
            }
        );
    }

    #[test]
    fn hard_links_add_names_and_entries_but_not_logical_bytes() {
        let sponsor = sponsor_with(limits(4, 10, 10));
        let first = commit_object(&sponsor, "first", 7);
        let second = path(&sponsor, "second");
        sponsor
            .prepare_hard_link(&first, &second)
            .unwrap()
            .commit()
            .unwrap();

        assert_eq!(
            sponsor.entry(&first).unwrap(),
            Some(FilesystemSponsorEntry::Object {
                extent: 7,
                names: 2,
                open_descriptors: 0,
            })
        );
        assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 7);
        sponsor.prepare_unlink(&first).unwrap().commit().unwrap();
        assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 7);
        assert_eq!(
            sponsor.entry(&second).unwrap(),
            Some(FilesystemSponsorEntry::Object {
                extent: 7,
                names: 1,
                open_descriptors: 0,
            })
        );
    }

    #[test]
    fn namespace_snapshot_exposes_groups_extents_and_quiescence() {
        let sponsor = sponsor_with(limits(5, 20, 20));
        commit_directory(&sponsor, "output");
        let first = commit_object(&sponsor, "output/first", 7);
        let second = path(&sponsor, "output/second");
        sponsor
            .prepare_hard_link(&first, &second)
            .unwrap()
            .commit()
            .unwrap();

        let descriptor = sponsor.prepare_open(&first).unwrap().commit().unwrap();
        let snapshot = sponsor.namespace_snapshot().unwrap();
        assert_eq!(snapshot.open_descriptors(), 1);
        assert!(!snapshot.transaction_prepared());
        assert_eq!(snapshot.entries().len(), 3);
        let groups = snapshot
            .entries()
            .iter()
            .filter_map(|entry| match entry.kind() {
                FilesystemSponsorNamespaceEntryKind::Object { group, extent } => {
                    assert_eq!(extent, 7);
                    Some(group)
                }
                FilesystemSponsorNamespaceEntryKind::Directory => None,
                FilesystemSponsorNamespaceEntryKind::Symlink { .. } => {
                    panic!("fixture has no symlink")
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], groups[1]);

        sponsor
            .prepare_close(&descriptor)
            .unwrap()
            .commit()
            .unwrap();
        assert_eq!(sponsor.namespace_snapshot().unwrap().open_descriptors(), 0);
    }

    #[test]
    fn rename_replacement_retains_an_open_replaced_object_until_close() {
        let sponsor = sponsor_with(limits(4, 20, 20));
        let source = commit_object(&sponsor, "source", 3);
        let destination = commit_object(&sponsor, "destination", 5);
        let replaced_descriptor = sponsor
            .prepare_open(&destination)
            .unwrap()
            .commit()
            .unwrap();

        sponsor
            .prepare_rename(&source, &destination)
            .unwrap()
            .commit()
            .unwrap();
        assert_eq!(
            sponsor.snapshot().unwrap(),
            FilesystemSponsorSnapshot {
                entries: 1,
                total_logical_bytes: 8,
                unique_objects: 2,
                open_descriptors: 1,
            }
        );
        assert_eq!(
            sponsor.entry(&destination).unwrap(),
            Some(FilesystemSponsorEntry::Object {
                extent: 3,
                names: 1,
                open_descriptors: 0,
            })
        );

        sponsor
            .prepare_close(&replaced_descriptor)
            .unwrap()
            .commit()
            .unwrap();
        assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 3);
        assert_eq!(sponsor.snapshot().unwrap().unique_objects, 1);
    }

    #[test]
    fn open_unlink_close_keeps_unnamed_object_charged_until_close() {
        let sponsor = sponsor_with(limits(2, 20, 20));
        let object = commit_object(&sponsor, "object", 10);
        let descriptor = sponsor.prepare_open(&object).unwrap().commit().unwrap();
        sponsor.prepare_unlink(&object).unwrap().commit().unwrap();

        assert_eq!(
            sponsor.snapshot().unwrap(),
            FilesystemSponsorSnapshot {
                entries: 0,
                total_logical_bytes: 10,
                unique_objects: 1,
                open_descriptors: 1,
            }
        );
        sponsor
            .prepare_close(&descriptor)
            .unwrap()
            .commit()
            .unwrap();
        assert_eq!(
            sponsor.snapshot().unwrap(),
            FilesystemSponsorSnapshot {
                entries: 0,
                total_logical_bytes: 0,
                unique_objects: 0,
                open_descriptors: 0,
            }
        );
    }

    #[test]
    fn create_object_open_commits_namespace_object_and_descriptor_together() {
        let sponsor = sponsor_with(limits(2, 20, 20));
        let object = path(&sponsor, "created-open");
        let prepared = sponsor.prepare_create_object_open(&object, 9).unwrap();

        assert_eq!(sponsor.entry(&object).unwrap(), None);
        assert_eq!(sponsor.snapshot().unwrap().open_descriptors, 0);
        let descriptor = prepared.commit().unwrap();
        assert_eq!(
            sponsor.entry(&object).unwrap(),
            Some(FilesystemSponsorEntry::Object {
                extent: 9,
                names: 1,
                open_descriptors: 1,
            })
        );
        assert_eq!(sponsor.snapshot().unwrap().open_descriptors, 1);

        sponsor
            .prepare_close(&descriptor)
            .unwrap()
            .commit()
            .unwrap();
    }

    #[test]
    fn open_with_extent_commits_truncation_and_descriptor_together() {
        let sponsor = sponsor_with(limits(2, 20, 20));
        let object = commit_object(&sponsor, "truncate-open", 13);
        let prepared = sponsor.prepare_open_with_extent(&object, Some(0)).unwrap();

        assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 13);
        let descriptor = prepared.commit().unwrap();
        assert_eq!(
            sponsor.entry(&object).unwrap(),
            Some(FilesystemSponsorEntry::Object {
                extent: 0,
                names: 1,
                open_descriptors: 1,
            })
        );
        assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 0);

        sponsor
            .prepare_close(&descriptor)
            .unwrap()
            .commit()
            .unwrap();
    }

    #[test]
    fn duplicate_commits_a_second_descriptor_for_the_same_unique_object() {
        let sponsor = sponsor_with(limits(2, 20, 20));
        let object = commit_object(&sponsor, "duplicate", 11);
        let first = sponsor.prepare_open(&object).unwrap().commit().unwrap();
        let prepared = sponsor.prepare_duplicate(&first).unwrap();

        assert_eq!(sponsor.snapshot().unwrap().open_descriptors, 1);
        let second = prepared.commit().unwrap();
        assert_ne!(first, second);
        assert_eq!(
            sponsor.entry(&object).unwrap(),
            Some(FilesystemSponsorEntry::Object {
                extent: 11,
                names: 1,
                open_descriptors: 2,
            })
        );
        assert_eq!(sponsor.snapshot().unwrap().unique_objects, 1);
        assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 11);

        sponsor.prepare_unlink(&object).unwrap().commit().unwrap();
        sponsor.prepare_close(&first).unwrap().commit().unwrap();
        assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 11);
        sponsor.prepare_close(&second).unwrap().commit().unwrap();
        assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 0);
    }

    #[test]
    fn partial_write_reserves_requested_growth_but_commits_actual_growth() {
        let sponsor = sponsor_with(limits(2, 10, 10));
        let object = commit_object(&sponsor, "object", 2);
        let descriptor = sponsor.prepare_open(&object).unwrap().commit().unwrap();

        sponsor
            .prepare_write(&descriptor, 2, 8)
            .unwrap()
            .commit_written(3)
            .unwrap();
        assert_eq!(
            sponsor.entry(&object).unwrap(),
            Some(FilesystemSponsorEntry::Object {
                extent: 5,
                names: 1,
                open_descriptors: 1,
            })
        );
        assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 5);

        let prepared = sponsor.prepare_write(&descriptor, 5, 5).unwrap();
        assert_eq!(
            prepared.commit_written(6).unwrap_err(),
            FilesystemSponsorError::PartialWriteExceedsPrepared {
                prepared: 5,
                actual: 6,
            }
        );
        assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 5);
    }

    #[test]
    fn failed_or_aborted_provider_mutation_never_commits_candidate_state() {
        let sponsor = sponsor_with(limits(4, 20, 20));
        let abandoned = path(&sponsor, "abandoned");
        let prepared = sponsor.prepare_create_object(&abandoned, 12).unwrap();
        assert_eq!(sponsor.snapshot().unwrap().entries, 0);
        assert_eq!(
            sponsor
                .prepare_create_directory(&path(&sponsor, "blocked"))
                .unwrap_err(),
            FilesystemSponsorError::TransactionAlreadyPrepared
        );
        prepared.abort();
        assert_eq!(sponsor.snapshot().unwrap().entries, 0);

        commit_directory(&sponsor, "committed");
        assert_eq!(sponsor.snapshot().unwrap().entries, 1);
        assert_eq!(sponsor.entry(&abandoned).unwrap(), None);
    }

    #[test]
    fn arithmetic_overflow_is_rejected_during_write_prepare() {
        let sponsor = sponsor_with(limits(2, u64::MAX, u64::MAX));
        let object = commit_object(&sponsor, "object", 0);
        let descriptor = sponsor.prepare_open(&object).unwrap().commit().unwrap();
        assert_eq!(
            sponsor.prepare_write(&descriptor, u64::MAX, 1).unwrap_err(),
            FilesystemSponsorError::ArithmeticOverflow
        );
        assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 0);
    }

    #[test]
    fn account_bound_paths_and_descriptors_reject_cross_account_operations() {
        let first = sponsor_with(limits(4, 20, 20));
        let second = FilesystemSponsor::with_limits("/other/session", limits(4, 20, 20)).unwrap();
        let first_object = commit_object(&first, "object", 1);
        let first_descriptor = first.prepare_open(&first_object).unwrap().commit().unwrap();
        let second_name = second.bind_path("/other/session/name").unwrap();

        assert_eq!(
            second
                .prepare_hard_link(&first_object, &second_name)
                .unwrap_err(),
            FilesystemSponsorError::CrossAccountOperation
        );
        assert_eq!(
            second
                .prepare_rename(&first_object, &second_name)
                .unwrap_err(),
            FilesystemSponsorError::CrossAccountOperation
        );
        assert_eq!(
            second.prepare_close(&first_descriptor).unwrap_err(),
            FilesystemSponsorError::CrossAccountOperation
        );
        assert_eq!(second.snapshot().unwrap().entries, 0);
    }

    #[test]
    fn directory_rename_moves_its_complete_namespace_subtree() {
        let sponsor = sponsor_with(limits(5, 10, 10));
        commit_directory(&sponsor, "old");
        let child = commit_object(&sponsor, "old/child", 4);
        let old = path(&sponsor, "old");
        let new = path(&sponsor, "new");
        sponsor
            .prepare_rename(&old, &new)
            .unwrap()
            .commit()
            .unwrap();

        assert_eq!(sponsor.entry(&old).unwrap(), None);
        assert_eq!(sponsor.entry(&child).unwrap(), None);
        assert_eq!(
            sponsor.entry(&path(&sponsor, "new/child")).unwrap(),
            Some(FilesystemSponsorEntry::Object {
                extent: 4,
                names: 1,
                open_descriptors: 0,
            })
        );
        assert_eq!(sponsor.snapshot().unwrap().entries, 2);
    }
}
