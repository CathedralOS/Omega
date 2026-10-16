//! Join native callback selections to the program Psi already produced.
//!
//! Every private ABI adapter names an exact callable in the shared artifact.
//! Neither another lowering run nor another Terminal identity is needed.

use crate::checked::CheckedCompilation;
use diagnostics::Diagnostic;
use std::sync::Arc;
use terminal_psi_to_abstract_operations::TerminalNativeRealizationProposal;

pub(crate) fn project_terminal_native_realization_proposal(
    checked: &CheckedCompilation,
    artifact: &Arc<terminal_codec::CanonicalTerminalArtifact>,
    checked_program_entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
    selections: &optimization_core::OptimizationSelections,
) -> Result<TerminalNativeRealizationProposal, Vec<Diagnostic>> {
    let callback_placements = checked.execution().callback_placements.as_slice();
    let callback_thunks = callback_placements
        .iter()
        .map(|placement| {
            let signature = psi_compiler::retained_callable_signature(
                &checked.execution().program,
                placement.selected_machine,
                checked.sources().exact_toolchain_sources(),
                artifact,
            )?;
            compilation_products::TerminalCallbackThunkArtifact::new(
                Arc::clone(artifact),
                signature.machine,
                &signature.scalar_source_positions,
                &signature.structural_source_positions,
            )
            .map_err(|message| vec![Diagnostic::error(message)])
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut proposal =
        terminal_psi_to_abstract_operations::project_terminal_native_realization_proposal(
            artifact,
            checked_program_entry,
            checked.terminal_native_proposal_request(
                callback_placements,
                callback_thunks,
                selections,
            )?,
        )?;
    if let Some(adapter) = checked
        .execution()
        .selected_program_entry
        .as_ref()
        .and_then(|entry| entry.physical_adapter())
    {
        let signature = psi_compiler::retained_callable_signature(
            &checked.execution().program,
            adapter,
            checked.sources().exact_toolchain_sources(),
            artifact,
        )?;
        proposal
            .bind_physical_adapter(artifact, signature.machine)
            .map_err(|message| vec![Diagnostic::error(message)])?;
    }
    Ok(proposal)
}
