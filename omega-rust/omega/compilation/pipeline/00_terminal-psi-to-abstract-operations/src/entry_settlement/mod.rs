//! Optimizer module role: executable entrance. Source-entry settlement entrance: pair the selected source declaration with
//! its calling contracts, replay the Terminal artifact, and retain owned custody.

mod calling_plans;
mod model;
mod physical_adapter;
mod service_establishment;

pub use physical_adapter::PhysicalProgramEntry;

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
    validate_receiver_eligibility(module, checked_entry, program_entry)?;
    service_establishment::validate_terminal_rows(
        module,
        entry,
        program_entry,
        checked_entry.receiver_eligibility(),
    )?;
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

/// Rejoin source context eligibility to its exact retained Terminal place.
/// Source positions are declaration ordinals, not dense structural ordinals.
fn validate_receiver_eligibility(
    module: &terminal_psi::TerminalModule,
    checked_entry: &CheckedProgramEntryTerminalReceipt,
    program_entry: NativeProgramEntrySettlement<'_>,
) -> Result<(), NativeProgramEntrySettlementError> {
    if let Some(eligibility) = checked_entry.receiver_eligibility() {
        // Only exact source Binding<...> fields require these rows.
        // Runtime erasure alone also covers ordinary proof fields, and cannot
        // classify a service. Retain completeness before self/ABI erasure can
        // bypass the physical receiver binder; existing replay below still
        // rejoins each row to Terminal fields and selected provider custody.
        let establishments = program_entry.fused_service_establishments();
        if establishments.len() != eligibility.fused_service_fields().len()
            || eligibility.fused_service_fields().iter().any(|field| {
                // Rows index the signature's plan-local table while the
                // eligibility receipt indexes the module's — join by text.
                establishments
                    .iter()
                    .filter(|row| {
                        module.identities.text(field.field_identity())
                            == program_entry.source.identities().text(row.field_identity())
                            && row.field_path().len() == field.field_path().len()
                            && row.field_path().iter().zip(field.field_path()).all(
                                |(row_segment, field_segment)| {
                                    module.identities.text(*field_segment)
                                        == program_entry.source.identities().text(*row_segment)
                                },
                            )
                            && module.identities.text(field.carrier_type_identity())
                                == program_entry
                                    .source
                                    .identities()
                                    .text(row.carrier_type_identity())
                    })
                    .count()
                    != 1
            })
        {
            return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
        }
        let entry = module
            .machine(checked_entry.terminal_entry())
            .ok_or(NativeProgramEntrySettlementError::ReceiverEligibilityDrift)?;
        let mut declarations = module
            .structural_types
            .iter()
            .filter(|declaration| declaration.id == eligibility.terminal_receiver_type());
        let declaration = declarations
            .next()
            .ok_or(NativeProgramEntrySettlementError::ReceiverEligibilityDrift)?;
        if declarations.next().is_some()
            || declaration.identity != eligibility.owned_receiver_type_identity()
            || entry
                .attachment
                .is_some_and(|attachment| attachment != eligibility.terminal_receiver_type())
            || program_entry
                .source
                .receiver()
                .normalized_type_identity()
                .and_then(|identity| program_entry.source.identities().text(identity))
                != module
                    .identities
                    .text(eligibility.source_receiver_type_identity())
        {
            return Err(NativeProgramEntrySettlementError::ReceiverEligibilityDrift);
        }
        if eligibility.cleanup()
            == terminal_psi::CheckedProgramEntryReceiverCleanup::OccupiesHostedExtent
            && !matches!(
                eligibility.projection(),
                terminal_psi::CheckedProgramEntryReceiverProjection::Retained { .. }
            )
        {
            // Cleanup occupancy needs a hosted extent to occupy: a receipt
            // that marks occupancy on an erased receiver is drift.
            return Err(NativeProgramEntrySettlementError::ReceiverEligibilityDrift);
        }
        match eligibility.projection() {
            terminal_psi::CheckedProgramEntryReceiverProjection::Retained {
                terminal_self,
                source_position,
            } => {
                let mut receivers = entry
                    .structural_parameters
                    .iter()
                    .filter(|parameter| parameter.place == terminal_self);
                let receiver = receivers
                    .next()
                    .ok_or(NativeProgramEntrySettlementError::ReceiverEligibilityDrift)?;
                if receivers.next().is_some()
                    || program_entry.source.receiver().source_position() != Some(source_position)
                    || receiver.access != terminal_psi::StructuralAccess::MutableBorrow
                    || receiver.structural_type != eligibility.terminal_receiver_type()
                {
                    return Err(NativeProgramEntrySettlementError::ReceiverEligibilityDrift);
                }
            }
            terminal_psi::CheckedProgramEntryReceiverProjection::Erased { source_position } => {
                if program_entry.source.receiver().source_position() != Some(source_position)
                    || entry.attachment != Some(eligibility.terminal_receiver_type())
                    || entry
                        .structural_parameters
                        .iter()
                        .any(|parameter| parameter.position == source_position)
                {
                    return Err(NativeProgramEntrySettlementError::ReceiverEligibilityDrift);
                }
            }
        }
    }
    Ok(())
}
