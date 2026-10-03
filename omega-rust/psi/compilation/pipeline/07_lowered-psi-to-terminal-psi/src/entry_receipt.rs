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

#[cfg(test)]
mod tests {
    use super::*;
    use checked_trees_to_lowered_psi::{
        TerminalMachineSelection, lower_machine, select_terminal_machine,
    };

    fn checked(source: &str) -> CheckedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
        )
        .unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        typed_trees_to_checked_trees::lower_typed_trees(
            typed,
            &typed_trees_to_checked_trees::CheckingRequest::settled(),
        )
        .unwrap()
    }

    #[test]
    fn receipt_requires_exactly_one_unit_entry() {
        let checked =
            checked("data Main { value: i32; } machine Main::run(&mut self) { self.value = 7; }");
        let machine = TerminalMachineSelection::Name("Main::run");
        let selection = select_terminal_machine(&checked, machine).unwrap();
        let lowered = lower_machine(&checked, machine).unwrap();
        prepare_program_entry_receipt(&checked, selection, [7; 32], &lowered).unwrap();
        let mut missing = lowered.clone();
        missing.semantic_module.machines.clear();
        assert!(matches!(
            prepare_program_entry_receipt(&checked, selection, [7; 32], &missing),
            Err(ProgramEntryTerminalReceiptError::TerminalEntryMultiplicity(
                0
            ))
        ));
        let mut duplicate = lowered;
        let entry = duplicate
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == duplicate.semantic_module.entry)
            .unwrap()
            .clone();
        duplicate.semantic_module.machines.push(entry);
        assert!(matches!(
            prepare_program_entry_receipt(&checked, selection, [7; 32], &duplicate),
            Err(ProgramEntryTerminalReceiptError::TerminalEntryMultiplicity(
                2
            ))
        ));
        let scalar_checked = self::checked("machine run() -> u64 { 17 }");
        let scalar_machine = TerminalMachineSelection::Name("run");
        let scalar_selection = select_terminal_machine(&scalar_checked, scalar_machine).unwrap();
        let scalar = lower_machine(&scalar_checked, scalar_machine).unwrap();
        assert!(matches!(
            prepare_program_entry_receipt(&scalar_checked, scalar_selection, [7; 32], &scalar),
            Err(ProgramEntryTerminalReceiptError::NonUnitEntry)
        ));
    }

    #[test]
    fn receipt_rejoins_only_its_published_semantics() {
        let checked =
            checked("data Main { value: i32; } machine Main::run(&mut self) { self.value = 7; }");
        let machine = TerminalMachineSelection::Name("Main::run");
        let selection = select_terminal_machine(&checked, machine).unwrap();
        let lowered = lower_machine(&checked, machine).unwrap();
        let receipt =
            prepare_program_entry_receipt(&checked, selection, [7; 32], &lowered).unwrap();
        let optimized =
            lowered_psi_to_lowered_psi::run_psi_optimization(lowered, Default::default()).unwrap();
        let artifact = crate::finalize_terminal_artifact(
            &optimized,
            checked.facts.proof.mathematical_signature.as_ref(),
        )
        .unwrap();
        rejoin_program_entry_receipt(&receipt, &artifact).unwrap();
        let mut different_identity = receipt.terminal_psi_identity();
        different_identity.program_fingerprint =
            terminal_psi::SemanticFingerprint::from_bytes([0; 32]);
        let substituted = CheckedProgramEntryTerminalReceipt::new(
            [7; 32],
            selection.name.clone(),
            selection.machine,
            different_identity,
            optimized.lowered().semantic_module.entry,
            None,
        );
        assert!(matches!(
            rejoin_program_entry_receipt(&substituted, &artifact),
            Err(ProgramEntryTerminalReceiptError::ArtifactSemanticIdentityMismatch)
        ));
    }
}
