//! Establish checked-source correspondence for the exact published entry.
//!
//! Source eligibility and Terminal identity are publication judgments, not
//! product-coordinator policy. A receipt does not grant native root authority.

use checked_trees::CheckedTrees;
use lowered_psi::LoweredPsi;
use terminal_codec::{CanonicalTerminalArtifact, terminal_psi_identity};
use terminal_psi::{CheckedProgramEntryTerminalReceipt, TerminalMachineResult};

mod receiver_eligibility;

#[derive(Debug)]
pub enum ProgramEntryTerminalReceiptError {
    TerminalEntryMultiplicity(usize),
    NonUnitEntry,
    TerminalIdentity(terminal_codec::CodecError),
    ArtifactSemanticIdentityMismatch,
}

impl std::fmt::Display for ProgramEntryTerminalReceiptError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProgramEntryTerminalReceiptError {}

/// Derive source correspondence for the optimized module's unique Unit entry.
/// The source-signature identity is supplied by the caller; this receipt does
/// not independently validate that signature or grant native entry authority.
pub fn prepare_program_entry_receipt(
    checked: &CheckedTrees,
    selection: &checked_trees::CheckedTerminalMachineSelection,
    source_signature_identity: [u8; 32],
    optimized_lowered: &LoweredPsi,
) -> Result<CheckedProgramEntryTerminalReceipt, ProgramEntryTerminalReceiptError> {
    let entry_matches = optimized_lowered
        .semantic_module
        .machines
        .iter()
        .filter(|machine| machine.id == optimized_lowered.semantic_module.entry)
        .collect::<Vec<_>>();
    let [entry] = entry_matches.as_slice() else {
        return Err(ProgramEntryTerminalReceiptError::TerminalEntryMultiplicity(
            entry_matches.len(),
        ));
    };
    if entry.result != TerminalMachineResult::Unit {
        return Err(ProgramEntryTerminalReceiptError::NonUnitEntry);
    }
    let identity = terminal_psi_identity(&optimized_lowered.semantic_module)
        .map_err(ProgramEntryTerminalReceiptError::TerminalIdentity)?;
    let receiver_eligibility =
        receiver_eligibility::derive(checked, selection, &optimized_lowered.semantic_module);
    Ok(CheckedProgramEntryTerminalReceipt::new(
        source_signature_identity,
        selection.name.clone(),
        selection.machine,
        identity,
        optimized_lowered.semantic_module.entry,
        receiver_eligibility,
    ))
}

/// Rejoin a prepared receipt to the exact semantics that publication retained.
/// This checks identity correspondence, not independently derived source evidence.
pub fn rejoin_program_entry_receipt(
    receipt: &CheckedProgramEntryTerminalReceipt,
    artifact: &CanonicalTerminalArtifact,
) -> Result<(), ProgramEntryTerminalReceiptError> {
    if artifact.manifest().semantic() != receipt.terminal_psi_identity() {
        return Err(ProgramEntryTerminalReceiptError::ArtifactSemanticIdentityMismatch);
    }
    Ok(())
}
