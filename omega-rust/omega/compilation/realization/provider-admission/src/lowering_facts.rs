// Callback and interrupt entries must retain the admission that checked them.
// Each cohort is minted by provider admission, bound to its Terminal program,
// and read through the sealed lowering facts produced by Omega 00.

use terminal_psi::TerminalPsiIdentity;

use crate::{AdmittedInstalledInterruptEntry, AdmittedNativeCallbackArgument};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedNativeCallbackCohort {
    pub(crate) psi: TerminalPsiIdentity,
    pub(crate) callbacks: Vec<AdmittedNativeCallbackArgument>,
}

impl AdmittedNativeCallbackCohort {
    pub const fn empty(psi: TerminalPsiIdentity) -> Self {
        Self {
            psi,
            callbacks: Vec::new(),
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
    pub(crate) entries: Vec<AdmittedInstalledInterruptEntry>,
    pub(crate) scopes: Vec<crate::AdmittedInstalledInterruptScope>,
    pub(crate) helpers: Vec<crate::AdmittedInstalledInterruptHelper>,
}

impl AdmittedInstalledInterruptCohort {
    pub const fn empty(psi: TerminalPsiIdentity) -> Self {
        Self {
            psi,
            entries: Vec::new(),
            scopes: Vec::new(),
            helpers: Vec::new(),
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
