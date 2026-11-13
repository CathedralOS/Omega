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

pub(crate) use terminal_codec::entry_receipt;

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
