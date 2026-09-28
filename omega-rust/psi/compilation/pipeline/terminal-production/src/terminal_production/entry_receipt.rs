//! Establish the checked-source receipt for the exact Terminal entry.

use checked_trees::CheckedTrees;
use lowered_psi::LoweredPsi;
use terminal_codec::terminal_psi_identity;
use terminal_psi::{CheckedProgramEntryTerminalReceipt, TerminalMachineResult};

use super::{ProgramEntryTerminalReceiptError, TerminalArtifactProductionError};
use crate::stage_timings::{TerminalProductionStage, TerminalProductionTimings};

mod receiver_eligibility;

/// Check the optimized module's unique Unit entry and derive the checked
/// `ProgramEntry` receipt before publication; the published artifact's
/// semantic identity is rejoined to the receipt afterwards.
pub(super) fn prepare_entry_receipt(
    checked: &CheckedTrees,
    selection: &checked_trees::CheckedTerminalMachineSelection,
    source_signature_identity: [u8; 32],
    optimized_lowered: &LoweredPsi,
    timings: &mut TerminalProductionTimings,
) -> Result<CheckedProgramEntryTerminalReceipt, TerminalArtifactProductionError> {
    timings.record_result(TerminalProductionStage::EntryReceipt, || {
        let entry_matches = optimized_lowered
            .semantic_module
            .machines
            .iter()
            .filter(|machine| machine.id == optimized_lowered.semantic_module.entry)
            .collect::<Vec<_>>();
        let [entry] = entry_matches.as_slice() else {
            return Err(TerminalArtifactProductionError::EntryReceipt(
                ProgramEntryTerminalReceiptError::TerminalEntryMultiplicity(entry_matches.len()),
            ));
        };
        if entry.result != TerminalMachineResult::Unit {
            return Err(TerminalArtifactProductionError::EntryReceipt(
                ProgramEntryTerminalReceiptError::NonUnitEntry,
            ));
        }
        Ok(())
    })?;
    let terminal_psi_identity = timings
        .record_result(TerminalProductionStage::TerminalIdentity, || {
            terminal_psi_identity(&optimized_lowered.semantic_module)
        })
        .map_err(ProgramEntryTerminalReceiptError::TerminalIdentity)
        .map_err(TerminalArtifactProductionError::EntryReceipt)?;
    let receiver_eligibility = timings
        .record_result(TerminalProductionStage::ReceiverEligibility, || {
            Ok::<_, ()>(receiver_eligibility::derive(
                checked,
                selection,
                &optimized_lowered.semantic_module,
            ))
        })
        .expect("receiver eligibility derivation is infallible");
    Ok(CheckedProgramEntryTerminalReceipt::new(
        source_signature_identity,
        selection.name.clone(),
        selection.machine,
        terminal_psi_identity,
        optimized_lowered.semantic_module.entry,
        receiver_eligibility,
    ))
}
