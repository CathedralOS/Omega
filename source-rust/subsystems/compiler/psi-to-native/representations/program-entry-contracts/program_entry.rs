//! The selected ProgramEntry declaration, target arrival contract and storage binding.
#![forbid(unsafe_code)]

mod entry_signature;
mod physical_contract;
mod storage_binding;

pub use entry_signature::{ProgramEntryExtentFieldRole, ProgramEntryResultSignature};
pub use entry_signature::{ProgramEntryExtentValueLayout, SelectedProgramEntrySignature};
pub use entry_signature::{ProgramEntrySignatureIdentity, ProgramEntryVisibleParameterSignature};
pub use physical_contract::ProgramEntryPhysicalContractPlan;
pub use target::entry_contract::ProgramEntryPhysicalContractPackageSourceDigest;
pub use storage_binding::{ProgramStorageEntryRootRole, SelectedProgramStorageEntryPlan};

/// The closed source receipt, physical contract and private-RAM recipes are
/// retained together through encoding. A receipt issuer owns their association.
pub trait HostedReceiverCustody {
    fn source(&self) -> &SelectedProgramEntrySignature;
    fn checked_entry(&self) -> &terminal_psi::CheckedProgramEntryTerminalReceipt;
    fn physical_contract(&self) -> &ProgramEntryPhysicalContractPlan;
    fn private_entry_ram(&self) -> &[psi_entry_memory::CheckedPrivateEntryRamRecipe];
}

/// The semantic entry and physical adapter selected from the checked program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalEntrySelection {
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub adapter: semantic_vocabulary::MachineId,
    pub semantic_entry: semantic_vocabulary::MachineId,
    pub inbound_ordinals: Vec<u32>,
}
