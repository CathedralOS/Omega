//! Establish checked-source correspondence for the exact published entry.
//!
//! Source eligibility and Terminal identity are publication judgments, not
//! product-coordinator policy. A receipt does not grant native root authority.

use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{
    CheckedProgramEntryTerminalReceipt, PreparedNominalRoot, TerminalMachineResult,
};

#[derive(Debug)]
pub enum ProgramEntryTerminalReceiptError {
    TerminalEntryMultiplicity(usize),
    NonUnitEntry,
    MissingEntryDeclaration,
    TerminalIdentity(terminal_codec::CodecError),
    ArtifactSemanticIdentityMismatch,
    ContextBridgeMultiplicity(usize),
    ContextBridgeOccurrenceMismatch,
}

impl std::fmt::Display for ProgramEntryTerminalReceiptError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProgramEntryTerminalReceiptError {}

impl From<terminal_psi::NominalRootError> for ProgramEntryTerminalReceiptError {
    fn from(error: terminal_psi::NominalRootError) -> Self {
        match error {
            terminal_psi::NominalRootError::MissingEntryDeclaration => {
                Self::MissingEntryDeclaration
            }
            terminal_psi::NominalRootError::TerminalEntryMultiplicity(count) => {
                Self::TerminalEntryMultiplicity(count)
            }
        }
    }
}

/// Bind the source-issued description to the sealed artifact's unique Unit
/// entry. The source-signature identity is caller-owned checked custody; this
/// operation neither reopens source nor grants native entry authority.
pub fn prepare_program_entry_receipt(
    declaration: &PreparedNominalRoot,
    source_signature_identity: [u8; 32],
    artifact: &CanonicalTerminalArtifact,
) -> Result<CheckedProgramEntryTerminalReceipt, ProgramEntryTerminalReceiptError> {
    let entry = declaration
        .resolve(artifact.semantic_module())
        .map_err(ProgramEntryTerminalReceiptError::from)?;
    if entry.result != TerminalMachineResult::Unit {
        return Err(ProgramEntryTerminalReceiptError::NonUnitEntry);
    }
    let source_declaration = entry
        .declaration
        .as_ref()
        .ok_or(ProgramEntryTerminalReceiptError::MissingEntryDeclaration)?;
    Ok(CheckedProgramEntryTerminalReceipt::new(
        source_signature_identity,
        declaration.source_machine_name().to_owned(),
        source_declaration.machine.clone(),
        artifact.manifest().semantic(),
        entry.id,
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
