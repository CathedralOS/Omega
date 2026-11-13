// A receipt joins an exact source declaration to one canonical artifact.
// It belongs to the artifact boundary, independently of which checker produced it.
use crate::CanonicalTerminalArtifact;
use terminal_psi::{
    CheckedProgramEntryTerminalReceipt, PreparedNominalRoot, TerminalMachineResult,
};

#[derive(Debug)]
pub enum ProgramEntryTerminalReceiptError {
    TerminalEntryMultiplicity(usize),
    NonUnitEntry,
    MissingEntryDeclaration,
    TerminalIdentity(crate::CodecError),
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

pub fn rejoin_program_entry_receipt(
    receipt: &CheckedProgramEntryTerminalReceipt,
    artifact: &CanonicalTerminalArtifact,
) -> Result<(), ProgramEntryTerminalReceiptError> {
    if artifact.manifest().semantic() != receipt.terminal_psi_identity() {
        return Err(ProgramEntryTerminalReceiptError::ArtifactSemanticIdentityMismatch);
    }
    Ok(())
}
