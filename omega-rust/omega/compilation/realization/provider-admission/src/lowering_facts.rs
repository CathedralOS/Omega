// Callback and interrupt entries must retain the admission that checked them.
// Each cohort is minted by provider admission, bound to its Terminal program,
// and read through the sealed lowering facts produced by Omega 00.

use std::sync::Arc;
use terminal_psi::TerminalPsiIdentity;

use crate::{AdmittedInstalledInterruptEntry, AdmittedNativeCallbackArgument};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedNativeCallbackCohort {
    pub(crate) psi: TerminalPsiIdentity,
    pub(crate) callbacks: Arc<[AdmittedNativeCallbackArgument]>,
}

impl AdmittedNativeCallbackCohort {
    pub fn empty(psi: TerminalPsiIdentity) -> Self {
        Self {
            psi,
            callbacks: Arc::from([]),
        }
    }

    pub const fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    pub fn callbacks(&self) -> &[AdmittedNativeCallbackArgument] {
        &self.callbacks
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedInstalledInterruptCohort {
    pub(crate) psi: TerminalPsiIdentity,
    pub(crate) entries: Arc<[AdmittedInstalledInterruptEntry]>,
    pub(crate) scopes: Arc<[crate::AdmittedInstalledInterruptScope]>,
    pub(crate) helpers: Arc<[crate::AdmittedInstalledInterruptHelper]>,
}

impl AdmittedInstalledInterruptCohort {
    pub fn empty(psi: TerminalPsiIdentity) -> Self {
        Self {
            psi,
            entries: Arc::from([]),
            scopes: Arc::from([]),
            helpers: Arc::from([]),
        }
    }

    pub const fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    pub fn entries(&self) -> &[AdmittedInstalledInterruptEntry] {
        &self.entries
    }

    pub fn scopes(&self) -> &[crate::AdmittedInstalledInterruptScope] {
        &self.scopes
    }

    pub fn helpers(&self) -> &[crate::AdmittedInstalledInterruptHelper] {
        &self.helpers
    }
}
