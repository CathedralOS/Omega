//! Establish checked-source correspondence for the exact published entry.
//!
//! Source eligibility and Terminal identity are publication judgments, not
//! product-coordinator policy. A receipt does not grant native root authority.

use checked_trees::CheckedTrees;
use lowered_psi::LoweredPsi;
use terminal_codec::{CanonicalTerminalArtifact, terminal_psi_identity};
use terminal_psi::{CheckedProgramEntryTerminalReceipt, TerminalMachineResult};

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

/// Derive source correspondence for the optimized module's unique Unit entry.
/// The source-signature identity is supplied by the caller; this receipt does
/// not independently validate that signature or grant native entry authority.
pub fn prepare_program_entry_receipt(
    checked: &CheckedTrees,
    selection: &checked_trees::CheckedTerminalMachineSelection,
    source_signature_identity: [u8; 32],
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    optimized_lowered: &LoweredPsi,
) -> Result<CheckedProgramEntryTerminalReceipt, ProgramEntryTerminalReceiptError> {
    // The receipt names one exact source member, while the immutable module
    // can be rooted at its independent portable family contract. Declaration
    // identity, not roster position or display spelling, rejoins that member.
    let entry = retained_source_machine(
        checked,
        selection.machine,
        exact_toolchain_sources,
        &optimized_lowered.semantic_module,
    )?;
    if entry.result != TerminalMachineResult::Unit {
        return Err(ProgramEntryTerminalReceiptError::NonUnitEntry);
    }
    let declaration = entry
        .declaration
        .as_ref()
        .ok_or(ProgramEntryTerminalReceiptError::MissingEntryDeclaration)?;
    let identity = terminal_psi_identity(&optimized_lowered.semantic_module)
        .map_err(ProgramEntryTerminalReceiptError::TerminalIdentity)?;
    Ok(CheckedProgramEntryTerminalReceipt::new(
        source_signature_identity,
        selection.name.clone(),
        declaration.machine.clone(),
        identity,
        entry.id,
        None,
    ))
}

/// Rejoin one checked declaration to its unique retained body. This identifies
/// an application subject, not permission to execute it or backing for inputs.
pub fn retained_source_machine<'module>(
    checked: &CheckedTrees,
    source: symbols::SymbolHandle,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    module: &'module terminal_psi::TerminalModule,
) -> Result<&'module terminal_psi::TerminalMachine, ProgramEntryTerminalReceiptError> {
    let declaration_identity = checked
        .exact_owner_nominal_identity(source, exact_toolchain_sources)
        .and_then(|identity| checked.type_identity_text(identity))
        .ok_or(ProgramEntryTerminalReceiptError::MissingEntryDeclaration)?;
    let entry_matches = module
        .machines
        .iter()
        .filter(|machine| {
            machine.declaration.as_ref().is_some_and(|declaration| {
                module.identities.text(declaration.machine.identity())
                    == Some(declaration_identity.as_str())
            })
        })
        .collect::<Vec<_>>();
    let [entry] = entry_matches.as_slice() else {
        return Err(ProgramEntryTerminalReceiptError::TerminalEntryMultiplicity(
            entry_matches.len(),
        ));
    };
    Ok(entry)
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
