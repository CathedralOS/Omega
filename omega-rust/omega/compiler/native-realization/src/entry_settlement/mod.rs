//! Optimizer module role: executable entrance. Source-entry settlement entrance: pair the selected source declaration with
//! its calling contracts, replay the Terminal artifact, and retain owned custody.

mod calling_plans;
mod model;
mod service_establishment;

pub use model::{
    NativeProgramEntrySettlement, NativeProgramEntrySettlementError,
    ValidatedNativeProgramEntrySettlement,
};

use terminal_production::CheckedProgramEntryTerminalReceipt;

/// Independently replay the complete source-signature, target, calling-plan,
/// Terminal-Psi, and entry-identity join without invoking the Psi receipt
/// producer.
pub fn validate_native_program_entry_settlement(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    checked_entry: &CheckedProgramEntryTerminalReceipt,
    program_entry: NativeProgramEntrySettlement<'_>,
    target: target::NativeTarget,
) -> Result<ValidatedNativeProgramEntrySettlement, NativeProgramEntrySettlementError> {
    let slot = program_entry.source.target_slot();
    if slot.owner.native_target() != target {
        return Err(NativeProgramEntrySettlementError::TargetDrift);
    }
    program_entry
        .validate_fused_service_establishments_for_target()
        .map_err(|_| NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift)?;
    program_entry
        .validate_for_target(target)
        .map_err(|_| NativeProgramEntrySettlementError::CallingPlanPairingDrift)?;
    if checked_entry.source_signature_identity() != program_entry.source.identity().bytes() {
        return Err(NativeProgramEntrySettlementError::SourceSignatureSubstitution);
    }
    if checked_entry.source_machine_name() != program_entry.source.machine_name() {
        return Err(NativeProgramEntrySettlementError::SourceMachineSubstitution);
    }
    artifact.validate().map_err(|error| {
        NativeProgramEntrySettlementError::CanonicalArtifactReplay(error.to_string())
    })?;
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).map_err(|error| {
        NativeProgramEntrySettlementError::CanonicalArtifactReplay(error.to_string())
    })?;
    let psi = terminal_codec::terminal_psi_identity(&module).map_err(|error| {
        NativeProgramEntrySettlementError::CanonicalArtifactReplay(error.to_string())
    })?;
    if psi != checked_entry.terminal_psi_identity()
        || artifact.manifest().semantic() != checked_entry.terminal_psi_identity()
    {
        return Err(NativeProgramEntrySettlementError::TerminalPsiSubstitution);
    }
    if module.entry != checked_entry.terminal_entry() {
        return Err(NativeProgramEntrySettlementError::TerminalEntrySubstitution);
    }
    let entry_count = module
        .machines
        .iter()
        .filter(|machine| machine.id == checked_entry.terminal_entry())
        .count();
    if entry_count != 1 {
        return Err(NativeProgramEntrySettlementError::TerminalEntryMultiplicity(entry_count));
    }
    service_establishment::validate_terminal_rows(&module, program_entry)?;
    Ok(ValidatedNativeProgramEntrySettlement {
        checked_entry: checked_entry.clone(),
        target,
        source: program_entry.source.clone(),
        semantic_boundary_entry_plan: program_entry.semantic_boundary_entry_plan.cloned(),
        storage_entry: program_entry.storage_entry.cloned(),
        fused_service_establishments: program_entry.fused_service_establishments.to_vec(),
    })
}

pub(crate) use service_establishment::validate_for_artifact_and_selected_plans as validate_fused_program_entry_establishments;
