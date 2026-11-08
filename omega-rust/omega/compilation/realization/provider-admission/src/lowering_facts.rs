// Admitted facts read by target lowering, and sealed callback and interrupt
// cohorts.
//
// Terminal-to-abstract lowering (Omega 00) keeps these facts in its sealed
// verified input. Abstract optimization (Omega 01) hands them to target
// lowering (Omega 02) as `AdmittedLoweringFacts` so target lowering and the
// backends after it read them without the full input. Native callback arguments
// and installed interrupt entries stay sealed: each cohort is minted only by
// provider admission, bound to the Terminal program it was admitted for, and
// verified by its consumers.

use installation_evidence::InstalledAssumedBindingEvidence;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_installation::AdmittedTargetMachineSelections;
use terminal_psi::{TerminalModule, TerminalPsiIdentity};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoweringEntryCall {
    pub boundary: BoundaryMachineId,
    pub entry: MachineId,
    pub source_signature: [u8; 32],
}

impl LoweringEntryCall {
    pub const fn boundary(&self) -> BoundaryMachineId {
        self.boundary
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub const fn source_signature(&self) -> [u8; 32] {
        self.source_signature
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedLoweringFacts {
    pub module: TerminalModule,
    pub program_entry: Option<LoweringEntryCall>,
    pub assumed_bindings: Vec<InstalledAssumedBindingEvidence>,
    pub target_machine_selections: Option<AdmittedTargetMachineSelections>,
    pub native_callbacks: AdmittedNativeCallbackCohort,
    pub installed_interrupts: AdmittedInstalledInterruptCohort,
}

impl AdmittedLoweringFacts {
    pub const fn module(&self) -> &TerminalModule {
        &self.module
    }

    pub const fn program_entry(&self) -> Option<&LoweringEntryCall> {
        self.program_entry.as_ref()
    }

    pub fn assumed_bindings(&self) -> &[InstalledAssumedBindingEvidence] {
        &self.assumed_bindings
    }

    pub const fn target_machine_selections(&self) -> Option<&AdmittedTargetMachineSelections> {
        self.target_machine_selections.as_ref()
    }

    pub const fn installed_interrupts(&self) -> &AdmittedInstalledInterruptCohort {
        &self.installed_interrupts
    }

    pub const fn native_callbacks(&self) -> &AdmittedNativeCallbackCohort {
        &self.native_callbacks
    }
}
