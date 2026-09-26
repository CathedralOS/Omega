//! Retirement authorities, receipts and retired installations — the endings
//! of an installed realization.
//!
//! `uninstall.rs` is the drain-or-quarantine join, `replacement.rs` the
//! patch-then-drain join, and `quarantine.rs` the trapping reservation a
//! drain that cannot complete parks in.

pub(crate) mod quarantine;
pub(crate) mod replacement;
pub(crate) mod uninstall;

use crate::artifacts::AdmittedArtifact;
use crate::authority_digests::RetirementFactDigest;
use crate::installation::InstalledCodeEvidence;
use crate::installation::{InstallationDiagnostic, InstalledCode};
use crate::placement::CodePlacement;

#[derive(Debug, PartialEq, Eq)]
pub struct RetirementAuthority {
    pub(crate) installed: InstalledCodeEvidence,
    pub(crate) required_facts: std::collections::BTreeSet<RetirementFactDigest>,
}

impl RetirementAuthority {
    pub fn from_admitted_provider(
        installed: &InstalledCode,
        required_facts: impl IntoIterator<Item = RetirementFactDigest>,
    ) -> Self {
        Self {
            installed: InstalledCodeEvidence::from_installed(installed),
            required_facts: required_facts.into_iter().collect(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct RetirementReceipt {
    pub(crate) installed: InstalledCodeEvidence,
    pub(crate) executors_quiesced: bool,
    pub(crate) execute_disabled: bool,
    pub(crate) write_authority_restored: bool,
    pub(crate) established_facts: std::collections::BTreeSet<RetirementFactDigest>,
}

impl RetirementReceipt {
    pub fn from_provider(
        installed: &InstalledCode,
        executors_quiesced: bool,
        execute_disabled: bool,
        write_authority_restored: bool,
        established_facts: impl IntoIterator<Item = RetirementFactDigest>,
    ) -> Self {
        Self {
            installed: InstalledCodeEvidence::from_installed(installed),
            executors_quiesced,
            execute_disabled,
            write_authority_restored,
            established_facts: established_facts.into_iter().collect(),
        }
    }
}

/// Result of synchronous retirement. The previous artifact remains reusable;
/// the exact destination returns to the W+NX `CodePlacement` state.
#[derive(Debug)]
pub struct RetiredInstallation {
    pub(crate) previous_artifact: AdmittedArtifact,
    pub(crate) placement: CodePlacement,
}

impl RetiredInstallation {
    pub const fn previous_artifact(&self) -> &AdmittedArtifact {
        &self.previous_artifact
    }

    pub fn into_placement(self) -> CodePlacement {
        self.placement
    }
}

#[derive(Debug)]
pub struct RetirementError {
    pub(crate) installed: InstalledCode,
    pub(crate) authority: RetirementAuthority,
    pub(crate) receipt: RetirementReceipt,
    pub(crate) diagnostic: InstallationDiagnostic,
}

impl RetirementError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (InstalledCode, RetirementAuthority, RetirementReceipt) {
        (self.installed, self.authority, self.receipt)
    }
}
