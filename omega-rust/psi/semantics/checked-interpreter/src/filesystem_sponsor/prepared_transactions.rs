//! Prepared mutations, opens, writes and account transactions.

use crate::filesystem_sponsor::accounts::{AccountState, FilesystemAccount, ObjectId};
use crate::filesystem_sponsor::{
    FilesystemOpenDescriptor, FilesystemSponsorError, FilesystemSponsorLimits,
};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct PreparedFilesystemMutation {
    pub(crate) prepared: PreparedAccountTransaction,
    pub(crate) candidate: Option<AccountState>,
}

#[derive(Debug)]
pub struct PreparedFilesystemOpen {
    pub(crate) prepared: PreparedAccountTransaction,
    pub(crate) candidate: Option<AccountState>,
    pub(crate) descriptor: FilesystemOpenDescriptor,
}

#[derive(Debug)]
pub struct PreparedFilesystemWrite {
    pub(crate) prepared: PreparedAccountTransaction,
    pub(crate) base: Option<AccountState>,
    pub(crate) object_id: ObjectId,
    pub(crate) offset: u64,
    pub(crate) prepared_bytes: u64,
    pub(crate) limits: FilesystemSponsorLimits,
}

#[derive(Debug)]
pub(crate) struct PreparedAccountTransaction {
    pub(crate) account: Arc<Mutex<FilesystemAccount>>,
    pub(crate) transaction_id: u64,
    pub(crate) active: bool,
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
    pub(crate) fn limits(&self) -> Result<FilesystemSponsorLimits, FilesystemSponsorError> {
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

    pub(crate) fn cancel_with(mut self, error: FilesystemSponsorError) -> FilesystemSponsorError {
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
