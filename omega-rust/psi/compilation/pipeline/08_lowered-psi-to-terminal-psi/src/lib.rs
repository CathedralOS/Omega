/*
We seal validated `PsiOptimizationStageResult` carriers into canonical Terminal
Psi artifacts (`finalize_terminal_program`), bind and rejoin source-entry
correspondence receipts (`prepare_program_entry_receipt` and
`rejoin_program_entry_receipt`), and verify boundary result policies
(`verify_boundary_result_policies`).
*/

/*
Our publication stage validates activation-storage custody across the complete
pre-Terminal CFG before constructing either a portable `PSITERM\0` artifact or
an invocation-local Terminal code carrier. Source-entry receipt preparation and
boundary result-policy verification remain separate from canonical artifact
sealing so source custody never substitutes for canonical Terminal semantics.
*/

/*
Stage 08 (`lowered-psi-to-terminal-psi`) is the final stage of the target-neutral
Psi pipeline, consuming `PsiOptimizationStageResult` from
`lowered-psi-to-lowered-psi` (Stage 07) and producing `TerminalProgramProduct`
for the Psi interpreter and Omega native realization.
*/

#![forbid(unsafe_code)]

mod result_policy_verification;

pub(crate) mod entry_receipt {
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
}

pub(crate) mod publish_artifact {
    use lowered_psi_to_lowered_psi::PsiOptimizationStageResult;

    pub fn finalize_terminal_program(
        optimized: PsiOptimizationStageResult,
        publication: terminal_codec::TerminalPublication,
    ) -> Result<
        terminal_codec::TerminalProgramProduct,
        terminal_codec::CanonicalTerminalArtifactError,
    > {
        let (lowered, execution) = optimized.into_parts();
        let (module, debug_map, _) = lowered.into_parts();
        terminal_semantics::activation_storage::validate_module(&module).map_err(|reason| {
            terminal_codec::CanonicalTerminalArtifactError::Semantic(
                terminal_codec::CodecError::MalformedStructuralFoundation(reason),
            )
        })?;
        match publication {
            terminal_codec::TerminalPublication::Portable => {
                let artifact = terminal_codec::CanonicalTerminalArtifact::from_parts(
                    module,
                    execution,
                    debug_map.as_ref(),
                )?;
                Ok(terminal_codec::TerminalProgramProduct::Portable(artifact))
            }
            terminal_codec::TerminalPublication::Invocation => {
                let code = terminal_codec::InvocationTerminalCode::new(module)
                    .map_err(terminal_codec::CanonicalTerminalArtifactError::Semantic)?;
                Ok(terminal_codec::TerminalProgramProduct::Invocation(code))
            }
        }
    }
}

pub use entry_receipt::{
    ProgramEntryTerminalReceiptError, prepare_program_entry_receipt, rejoin_program_entry_receipt,
};
pub use publish_artifact::finalize_terminal_program;
pub use result_policy_verification::verify_boundary_result_policies;
