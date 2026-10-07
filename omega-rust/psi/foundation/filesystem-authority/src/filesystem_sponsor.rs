#![forbid(unsafe_code)]

//! Resource accounting for private append-and-seal staging.
//!
//! The host tree is the namespace. The account keeps entry/byte counters and
//! only live writer custody; closing releases the path and extent. No object
//! graph, namespace snapshot, state clone or divergence replay remains.
//! Exclusive creation, append-only writes and no removal/rename/links mean
//! retained storage only grows until disposal, so counters suffice. Ordinary
//! real-host grants use their provider without this staging sponsor.
//!
//! Tokens reserve growth before mutation and charge after host success. The
//! provider must create exclusively and keep the sole writer until closing.
//! Private directory custody supplies the documented isolation premise; host
//! preflight never follows a staging symlink. This is not grant admission.

mod accounts;
mod handles;
mod prepared_transactions;
mod private_staging;
mod sponsor_errors;
use accounts::{
    FilesystemAccount, StagingWriter, check_account_path, checked_add, normalize_absolute,
};
pub use handles::{FilesystemOpenDescriptor, FilesystemSponsorEntry, FilesystemSponsorPath};
use prepared_transactions::{PreparedAccountTransaction, StagingChange};
pub use prepared_transactions::{
    PreparedFilesystemMutation, PreparedFilesystemOpen, PreparedFilesystemWrite,
};
pub use sponsor_errors::FilesystemSponsorError;
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
                entries: 0,
                logical_bytes: 0,
                writers: vec![None],
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
    /// Bind a strict descendant lexically; operation preflight checks host kinds.
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
    /// Inspect one actual host entry, without retaining a namespace model.
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
    /// Exclusive creation is the only write-open in the staging protocol.
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
    /// Readers may observe sealed files, never a live writer's mutable bytes.
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
    /// Reserve append growth; repositioned writes and truncation are unsupported.
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
