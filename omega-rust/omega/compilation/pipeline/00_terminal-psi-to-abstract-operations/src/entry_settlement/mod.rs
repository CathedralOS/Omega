//! Optimizer module role: executable entrance. Source-entry settlement entrance: pair the selected source declaration with
//! its calling contracts, replay the Terminal artifact, and retain owned custody.

mod calling_plans;
mod model;
mod physical_adapter;
mod semantic_call;

pub use physical_adapter::PhysicalProgramEntry;
pub use semantic_call::SelectedProgramEntryCall;

pub use model::{
    NativeProgramEntrySettlement, NativeProgramEntrySettlementError,
    ValidatedNativeProgramEntrySettlement,
};

use terminal_psi::CheckedProgramEntryTerminalReceipt;

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
    let admitted_calling_plans = program_entry
        .validate_for_target(target)
        .map_err(|_| NativeProgramEntrySettlementError::CallingPlanPairingDrift)?;
    if checked_entry.source_signature_identity() != program_entry.source.identity().bytes() {
        return Err(NativeProgramEntrySettlementError::SourceSignatureSubstitution);
    }
    // The source owner closes arena coordinates into exact package/toolchain
    // declaration identity. Display names alone cannot distinguish packages.
    let module = artifact.semantic_module();
    if module
        .identities
        .text(checked_entry.declaration().identity())
        != Some(program_entry.source.declaration())
    {
        return Err(NativeProgramEntrySettlementError::SourceMachineSubstitution);
    }
    let psi = terminal_codec::terminal_psi_identity(module).map_err(|error| {
        NativeProgramEntrySettlementError::CanonicalArtifactReplay(error.to_string())
    })?;
    if psi != checked_entry.terminal_psi_identity()
        || artifact.manifest().semantic() != checked_entry.terminal_psi_identity()
        || program_entry
            .physical_adapter
            .is_some_and(|adapter| adapter.terminal() != psi)
    {
        return Err(NativeProgramEntrySettlementError::TerminalPsiSubstitution);
    }
    let selections = terminal_installation::admit_target_machine_selections(
        artifact.view(),
        slot.owner.target_name(),
    );
    let entry = selections
        .entry(
            artifact.view().identity(),
            module,
            terminal_psi::TargetMachineCallable::Machine(checked_entry.terminal_entry()),
        )
        .map_err(|error| NativeProgramEntrySettlementError::TargetFamily(error.to_string()))?;
    let entry_count = module
        .machines
        .iter()
        .filter(|machine| machine.id == checked_entry.terminal_entry())
        .count();
    if entry_count != 1 {
        return Err(NativeProgramEntrySettlementError::TerminalEntryMultiplicity(entry_count));
    }
    if module
        .machine(checked_entry.terminal_entry())
        .and_then(|machine| machine.declaration.as_ref())
        .is_none_or(|declaration| declaration.machine != *checked_entry.declaration())
    {
        return Err(NativeProgramEntrySettlementError::SourceMachineSubstitution);
    }
    // These are actual selected boundary inputs, not module.entry discovery.
    // The public source declaration and selected realization both retain
    // their full erased-input obligation when they are distinct coordinates.
    for machine in [checked_entry.terminal_entry(), entry] {
        terminal_semantics::validate_empty_erased_invocation_inputs(module, machine).map_err(
            |reason| NativeProgramEntrySettlementError::UnsuppliedErasedInputs { machine, reason },
        )?;
    }
    Ok(ValidatedNativeProgramEntrySettlement {
        checked_entry: checked_entry.clone(),
        physical_adapter: program_entry.physical_adapter.cloned(),
        target,
        source: program_entry.source.clone(),
        semantic_calling_application: program_entry.semantic_calling_application.cloned(),
        admitted_calling_plans,
        physical_calling_application: program_entry.physical_calling_application.cloned(),
        storage_entry: program_entry.storage_entry.cloned(),
        fused_service_establishments: program_entry.fused_service_establishments.to_vec(),
        // Executable input supplies these loans, not declaration settlement.
        placed_view_establishments: Vec::new(),
        private_entry_ram: Vec::new(),
    })
}
