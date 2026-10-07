//! Reserve growth before host mutation and charge the actual result afterwards.
//!
//! A token owns a resource delta, never a cloned filesystem state. Dropping a
//! failed host operation releases its reservation without charging counters.

use super::accounts::{FilesystemAccount, StagingWriter, checked_add};
use super::{FilesystemOpenDescriptor, FilesystemSponsorError};
use std::sync::{Arc, Mutex};

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
