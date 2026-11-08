/*
We enforce resource accounting and private directory custody for append-and-seal
staging sessions. Our `FilesystemSponsor` reserves entry and byte growth before
each host mutation, tracks live writer descriptors until sealing on close, and
manages isolated private staging directories without retaining an in-memory
namespace snapshot.
*/

use std::error::Error;
use std::fmt;
use std::io;
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
pub struct FilesystemSponsorPath {
    pub(crate) account_id: u64,
    pub(crate) relative: PathBuf,
}

impl FilesystemSponsorPath {
    pub fn relative(&self) -> &Path {
        &self.relative
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemOpenDescriptor {
    pub(crate) account_id: u64,
    pub(crate) writer_id: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemSponsorEntry {
    Directory,
    Symlink { spelling_bytes: u64 },
    Object { extent: u64 },
}

#[derive(Debug)]
pub(crate) struct FilesystemAccount {
    pub(crate) id: u64,
    pub(crate) session_root: PathBuf,
    pub(crate) limits: FilesystemSponsorLimits,
    pub(crate) entries: u64,
    pub(crate) logical_bytes: u64,
    pub(crate) writers: Vec<Option<StagingWriter>>, // Live writers; closing releases path and extent.
    pub(crate) prepared_transaction: Option<u64>,
    pub(crate) next_transaction_id: u64,
}

#[derive(Debug)]
pub(crate) struct StagingWriter {
    pub(crate) relative_path: PathBuf,
    pub(crate) extent: u64,
}

impl FilesystemAccount {
    pub(crate) fn admit_growth(
        &self,
        entries: u64,
        bytes: u64,
        extent: u64,
    ) -> Result<(), FilesystemSponsorError> {
        let attempted = checked_add(self.entries, entries)?;
        if attempted > self.limits.maximum_entries {
            return Err(FilesystemSponsorError::EntryLimitExceeded {
                limit: self.limits.maximum_entries,
                attempted,
            });
        }
        let attempted = checked_add(self.logical_bytes, bytes)?;
        if attempted > self.limits.maximum_total_logical_bytes {
            return Err(FilesystemSponsorError::TotalLogicalBytesLimitExceeded {
                limit: self.limits.maximum_total_logical_bytes,
                attempted,
            });
        }
        if extent > self.limits.maximum_object_extent {
            return Err(FilesystemSponsorError::ObjectExtentLimitExceeded {
                limit: self.limits.maximum_object_extent,
                attempted: extent,
            });
        }
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

#[derive(Debug)]
pub(crate) enum StagingChange {
    Directory,
    CreateWriter(StagingWriter),
    CloseWriter(usize),
    Append { writer_id: usize, bytes: u64 },
}

#[derive(Debug)]
pub(crate) struct PreparedAccountTransaction {
    pub(crate) account: Arc<Mutex<FilesystemAccount>>,
    pub(crate) transaction_id: u64,
    pub(crate) change: Option<StagingChange>,
}

#[derive(Debug)]
pub struct PreparedFilesystemMutation {
    pub(crate) prepared: PreparedAccountTransaction,
}

#[derive(Debug)]
pub struct PreparedFilesystemOpen {
    pub(crate) prepared: PreparedAccountTransaction,
}

#[derive(Debug)]
pub struct PreparedFilesystemWrite {
    pub(crate) prepared: PreparedAccountTransaction,
}

impl PreparedFilesystemMutation {
    pub fn commit(self) -> Result<(), FilesystemSponsorError> {
        self.prepared.commit(None).map(|_| ())
    }
    pub fn abort(self) {}
}

impl PreparedFilesystemOpen {
    pub fn commit(self) -> Result<FilesystemOpenDescriptor, FilesystemSponsorError> {
        self.prepared
            .commit(None)?
            .ok_or(FilesystemSponsorError::TransactionNoLongerCurrent)
    }
    pub fn abort(self) {}
}

impl PreparedFilesystemWrite {
    pub fn commit_written(self, bytes: u64) -> Result<(), FilesystemSponsorError> {
        self.prepared.commit(Some(bytes)).map(|_| ())
    }
    pub fn abort(self) {}
}

impl PreparedAccountTransaction {
    fn commit(
        mut self,
        actual_bytes: Option<u64>,
    ) -> Result<Option<FilesystemOpenDescriptor>, FilesystemSponsorError> {
        let mut account = self
            .account
            .lock()
            .map_err(|_| FilesystemSponsorError::AccountPoisoned)?;
        if account.prepared_transaction != Some(self.transaction_id) {
            return Err(FilesystemSponsorError::TransactionNoLongerCurrent);
        }
        let change = self
            .change
            .take()
            .ok_or(FilesystemSponsorError::TransactionNoLongerCurrent)?;
        let descriptor = match change {
            StagingChange::Directory => {
                account.entries = checked_add(account.entries, 1)?;
                None
            }
            StagingChange::CreateWriter(writer) => {
                let writer_id = account.writers.len();
                account.entries = checked_add(account.entries, 1)?;
                account.logical_bytes = checked_add(account.logical_bytes, writer.extent)?;
                account.writers.push(Some(writer));
                Some(FilesystemOpenDescriptor {
                    account_id: account.id,
                    writer_id,
                })
            }
            StagingChange::CloseWriter(writer_id) => {
                account
                    .writers
                    .get_mut(writer_id)
                    .and_then(Option::take)
                    .ok_or(FilesystemSponsorError::OpenDescriptorNotFound)?;
                None
            }
            StagingChange::Append { writer_id, bytes } => {
                let actual =
                    actual_bytes.ok_or(FilesystemSponsorError::TransactionNoLongerCurrent)?;
                if actual > bytes {
                    return Err(FilesystemSponsorError::PartialWriteExceedsPrepared {
                        prepared: bytes,
                        actual,
                    });
                }
                let writer = account
                    .writers
                    .get_mut(writer_id)
                    .and_then(Option::as_mut)
                    .ok_or(FilesystemSponsorError::OpenDescriptorNotFound)?;
                writer.extent = checked_add(writer.extent, actual)?;
                account.logical_bytes = checked_add(account.logical_bytes, actual)?;
                None
            }
        };
        account.prepared_transaction = None;
        Ok(descriptor)
    }
}

impl Drop for PreparedAccountTransaction {
    fn drop(&mut self) {
        if let Ok(mut account) = self.account.lock()
            && account.prepared_transaction == Some(self.transaction_id)
        {
            account.prepared_transaction = None;
        }
    }
}

#[derive(Debug)]
struct PrivateStaging {
    root: PathBuf,
    state: Mutex<StagingState>,
}

#[derive(Debug)]
struct StagingState {
    live: bool,
    removed: bool,
}

impl PrivateStaging {
    fn dispose(&self) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("private staging custody lock poisoned"))?;
        state.live = false;
        if state.removed {
            return Ok(());
        }
        match std::fs::remove_dir_all(&self.root) {
            Ok(()) => state.removed = true,
            Err(error) if error.kind() == io::ErrorKind::NotFound => state.removed = true,
            Err(error) => return Err(error),
        }
        Ok(())
    }
}

impl Drop for PrivateStaging {
    fn drop(&mut self) {
        let _ = self.dispose();
    }
}

#[derive(Debug, Clone)]
pub struct FilesystemSponsor {
    account: Arc<Mutex<FilesystemAccount>>,
    private_staging: Option<Arc<PrivateStaging>>,
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
                entries: 0,
                logical_bytes: 0,
                writers: vec![None],
                prepared_transaction: None,
                next_transaction_id: 1,
            })),
        })
    }

    pub fn create_private(
        path: impl AsRef<Path>,
        limits: FilesystemSponsorLimits,
    ) -> io::Result<Self> {
        let path = path.as_ref();
        let name = path.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "private staging requires a directory leaf",
            )
        })?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let parent = std::fs::canonicalize(parent)?;
        let root = parent.join(name);
        #[cfg_attr(not(unix), allow(unused_mut))]
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&root)?;
        let owner = Arc::new(PrivateStaging {
            root: root.clone(),
            state: Mutex::new(StagingState {
                live: true,
                removed: false,
            }),
        });
        let canonical = std::fs::canonicalize(&root)?;
        if canonical != root || canonical.parent() != Some(parent.as_path()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "private staging escaped its canonical parent",
            ));
        }
        let mut sponsor = Self::with_limits(&canonical, limits).map_err(io::Error::other)?;
        sponsor.private_staging = Some(owner);
        Ok(sponsor)
    }

    pub fn owns_private_staging_root(&self, root: &Path) -> bool {
        let Some(owner) = &self.private_staging else {
            return false;
        };
        let Ok(root) = normalize_absolute(root) else {
            return false;
        };
        owner
            .state
            .lock()
            .is_ok_and(|state| state.live && root != owner.root && root.starts_with(&owner.root))
    }

    pub fn dispose_private_staging(&self) -> io::Result<()> {
        match &self.private_staging {
            Some(owner) => owner.dispose(),
            None => Ok(()),
        }
    }

    pub fn limits(&self) -> Result<FilesystemSponsorLimits, FilesystemSponsorError> {
        Ok(self.lock_account()?.limits)
    }

    pub fn session_root(&self) -> Result<PathBuf, FilesystemSponsorError> {
        Ok(self.lock_account()?.session_root.clone())
    }

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

    pub fn entry(
        &self,
        path: &FilesystemSponsorPath,
    ) -> Result<Option<FilesystemSponsorEntry>, FilesystemSponsorError> {
        let absolute = self.host_path(path)?;
        match std::fs::symlink_metadata(&absolute) {
            Ok(metadata) if metadata.is_dir() => Ok(Some(FilesystemSponsorEntry::Directory)),
            Ok(metadata) if metadata.is_file() => Ok(Some(FilesystemSponsorEntry::Object {
                extent: metadata.len(),
            })),
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let target = std::fs::read_link(&absolute).map_err(backing_error)?;
                let spelling_bytes = u64::try_from(target.as_os_str().as_encoded_bytes().len())
                    .map_err(|_| FilesystemSponsorError::ArithmeticOverflow)?;
                Ok(Some(FilesystemSponsorEntry::Symlink { spelling_bytes }))
            }
            Ok(_) => Err(FilesystemSponsorError::EntryIsNotRegularObject(
                path.relative.clone(),
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(backing_error(error)),
        }
    }

    pub fn prepare_create_directory(
        &self,
        path: &FilesystemSponsorPath,
    ) -> Result<PreparedFilesystemMutation, FilesystemSponsorError> {
        self.require_fresh_path(path)?;
        Ok(PreparedFilesystemMutation {
            prepared: self.reserve(self.lock_account()?, StagingChange::Directory, 1, 0, 0)?,
        })
    }

    pub fn prepare_create_object_open(
        &self,
        path: &FilesystemSponsorPath,
        initial_extent: u64,
    ) -> Result<PreparedFilesystemOpen, FilesystemSponsorError> {
        self.require_fresh_path(path)?;
        Ok(PreparedFilesystemOpen {
            prepared: self.reserve(
                self.lock_account()?,
                StagingChange::CreateWriter(StagingWriter {
                    relative_path: path.relative.clone(),
                    extent: initial_extent,
                }),
                1,
                initial_extent,
                initial_extent,
            )?,
        })
    }

    pub fn require_sealed(
        &self,
        path: &FilesystemSponsorPath,
    ) -> Result<(), FilesystemSponsorError> {
        self.host_path(path)?;
        if self
            .lock_account()?
            .writers
            .iter()
            .flatten()
            .any(|writer| writer.relative_path == path.relative)
        {
            return Err(FilesystemSponsorError::UnsealedEntry(path.relative.clone()));
        }
        Ok(())
    }

    pub fn prepare_close(
        &self,
        descriptor: &FilesystemOpenDescriptor,
    ) -> Result<PreparedFilesystemMutation, FilesystemSponsorError> {
        let account = self.lock_account()?;
        self.writer(&account, descriptor)?;
        Ok(PreparedFilesystemMutation {
            prepared: self.reserve(
                account,
                StagingChange::CloseWriter(descriptor.writer_id),
                0,
                0,
                0,
            )?,
        })
    }

    pub fn prepare_write(
        &self,
        descriptor: &FilesystemOpenDescriptor,
        offset: u64,
        requested_bytes: u64,
    ) -> Result<PreparedFilesystemWrite, FilesystemSponsorError> {
        let account = self.lock_account()?;
        let extent = self.writer(&account, descriptor)?.extent;
        if offset != extent {
            return Err(FilesystemSponsorError::UnsupportedStagingOperation(
                "non-append write",
            ));
        }
        let end = checked_add(extent, requested_bytes)?;
        Ok(PreparedFilesystemWrite {
            prepared: self.reserve(
                account,
                StagingChange::Append {
                    writer_id: descriptor.writer_id,
                    bytes: requested_bytes,
                },
                0,
                requested_bytes,
                end,
            )?,
        })
    }

    fn writer<'account>(
        &self,
        account: &'account FilesystemAccount,
        descriptor: &FilesystemOpenDescriptor,
    ) -> Result<&'account StagingWriter, FilesystemSponsorError> {
        if account.id != descriptor.account_id {
            return Err(FilesystemSponsorError::CrossAccountOperation);
        }
        account
            .writers
            .get(descriptor.writer_id)
            .and_then(Option::as_ref)
            .ok_or(FilesystemSponsorError::OpenDescriptorNotFound)
    }

    fn reserve(
        &self,
        mut account: MutexGuard<'_, FilesystemAccount>,
        change: StagingChange,
        entries: u64,
        bytes: u64,
        extent: u64,
    ) -> Result<PreparedAccountTransaction, FilesystemSponsorError> {
        if account.prepared_transaction.is_some() {
            return Err(FilesystemSponsorError::TransactionAlreadyPrepared);
        }
        account.admit_growth(entries, bytes, extent)?;
        let transaction_id = account.next_transaction_id;
        account.next_transaction_id = checked_add(transaction_id, 1)?;
        account.prepared_transaction = Some(transaction_id);
        Ok(PreparedAccountTransaction {
            account: Arc::clone(&self.account),
            transaction_id,
            change: Some(change),
        })
    }

    fn host_path(&self, path: &FilesystemSponsorPath) -> Result<PathBuf, FilesystemSponsorError> {
        let account = self.lock_account()?;
        check_account_path(account.id, path)?;
        let mut absolute = account.session_root.clone();
        drop(account);
        let root = std::fs::symlink_metadata(&absolute).map_err(backing_error)?;
        if !root.is_dir() || root.file_type().is_symlink() {
            return Err(FilesystemSponsorError::ParentIsNotDirectory(absolute));
        }
        let mut components = path.relative.components().peekable();
        while let Some(component) = components.next() {
            let Component::Normal(name) = component else {
                return Err(FilesystemSponsorError::PathEscapesFilesystemRoot(
                    path.relative.clone(),
                ));
            };
            absolute.push(name);
            if components.peek().is_none() {
                break;
            }
            match std::fs::symlink_metadata(&absolute) {
                Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
                Ok(_) => return Err(FilesystemSponsorError::ParentIsNotDirectory(absolute)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Err(FilesystemSponsorError::ParentEntryMissing(absolute));
                }
                Err(error) => return Err(backing_error(error)),
            }
        }
        Ok(absolute)
    }

    fn require_fresh_path(
        &self,
        path: &FilesystemSponsorPath,
    ) -> Result<(), FilesystemSponsorError> {
        if self.entry(path)?.is_some() {
            return Err(FilesystemSponsorError::EntryAlreadyExists(
                path.relative.clone(),
            ));
        }
        Ok(())
    }

    fn lock_account(&self) -> Result<MutexGuard<'_, FilesystemAccount>, FilesystemSponsorError> {
        self.account
            .lock()
            .map_err(|_| FilesystemSponsorError::AccountPoisoned)
    }
}

fn backing_error(error: std::io::Error) -> FilesystemSponsorError {
    FilesystemSponsorError::BackingStore(error.to_string())
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
    EntryIsNotRegularObject(PathBuf),
    UnsealedEntry(PathBuf),
    UnsupportedStagingOperation(&'static str),
    BackingStore(String),
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
                write!(
                    formatter,
                    "parent entry does not exist: {}",
                    path.to_string_lossy().replace('\\', "/")
                )
            }
            Self::ParentIsNotDirectory(path) => {
                write!(
                    formatter,
                    "parent entry is not a directory: {}",
                    path.to_string_lossy().replace('\\', "/")
                )
            }
            Self::EntryAlreadyExists(path) => {
                write!(
                    formatter,
                    "entry already exists: {}",
                    path.to_string_lossy().replace('\\', "/")
                )
            }
            Self::EntryIsNotRegularObject(path) => {
                write!(
                    formatter,
                    "entry is not a regular object: {}",
                    path.to_string_lossy().replace('\\', "/")
                )
            }
            Self::UnsealedEntry(path) => {
                write!(
                    formatter,
                    "staging entry is not sealed: {}",
                    path.to_string_lossy().replace('\\', "/")
                )
            }
            Self::UnsupportedStagingOperation(operation) => {
                write!(formatter, "staging protocol does not support {operation}")
            }
            Self::BackingStore(message) => {
                write!(formatter, "staging backing store failed: {message}")
            }
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
