//! Optimizer module role: executable entrance. Source-entry settlement entrance: pair the selected source declaration with
//! its calling contracts, replay the Terminal artifact, and retain owned custody.

mod calling_plans;
mod model;
mod service_establishment;

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
    program_entry
        .validate_for_target(target)
        .map_err(|_| NativeProgramEntrySettlementError::CallingPlanPairingDrift)?;
    if checked_entry.source_signature_identity() != program_entry.source.identity().bytes() {
        return Err(NativeProgramEntrySettlementError::SourceSignatureSubstitution);
    }
    // Settle the exact selected machine symbol, not a display name: a build
    // product operand resolved lexically can legitimately share its qualified
    // name spelling with a declaration in another package.
    if checked_entry.source_machine_symbol() != program_entry.source.machine_symbol() {
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
    if let Some(eligibility) = checked_entry.receiver_eligibility() {
        // Only exact source Service<...> fields require these rows.
        // Runtime erasure alone also covers ordinary proof fields, and cannot
        // classify a service. Retain completeness before self/ABI erasure can
        // bypass the physical receiver binder; existing replay below still
        // rejoins each row to Terminal fields and selected provider custody.
        let establishments = program_entry.fused_service_establishments();
        if establishments.len() != eligibility.fused_service_fields().len()
            || eligibility.fused_service_fields().iter().any(|field| {
                establishments
                    .iter()
                    .filter(|row| {
                        row.field_identity() == field.field_identity()
                            && row.carrier_type_identity() == field.carrier_type_identity()
                    })
                    .count()
                    != 1
            })
        {
            return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
        }
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
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
            || entry.attachment != Some(eligibility.terminal_receiver_type())
            || program_entry.source.receiver().normalized_type_identity()
                != Some(eligibility.source_receiver_type_identity())
        {
            return Err(NativeProgramEntrySettlementError::ReceiverEligibilityDrift);
        }
        let mut receivers = entry
            .structural_parameters
            .iter()
            .filter(|parameter| parameter.is_self);
        match eligibility.projection() {
            terminal_psi::CheckedProgramEntryReceiverProjection::Retained {
                terminal_self,
                source_position,
            } => {
                let receiver = receivers
                    .next()
                    .ok_or(NativeProgramEntrySettlementError::ReceiverEligibilityDrift)?;
                if receivers.next().is_some()
                    || receiver.place != terminal_self
                    || receiver.position != source_position
                    || receiver.access != terminal_psi::StructuralAccess::MutableBorrow
                    || receiver.structural_type != eligibility.terminal_receiver_type()
                {
                    return Err(NativeProgramEntrySettlementError::ReceiverEligibilityDrift);
                }
            }
            terminal_psi::CheckedProgramEntryReceiverProjection::Erased { source_position } => {
                if receivers.next().is_some()
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
    service_establishment::validate_terminal_rows(&module, program_entry)?;
    Ok(ValidatedNativeProgramEntrySettlement {
        checked_entry: checked_entry.clone(),
        target,
        source: program_entry.source.clone(),
        semantic_calling_application: program_entry.semantic_calling_application.cloned(),
        physical_calling_application: program_entry.physical_calling_application.cloned(),
        storage_entry: program_entry.storage_entry.cloned(),
        fused_service_establishments: program_entry.fused_service_establishments.to_vec(),
        // Placed-view loans are executable-input custody, not program-entry
        // declaration custody: they attach only when a bound input reopens.
        placed_view_establishments: Vec::new(),
    })
}

pub(crate) use service_establishment::validate_for_artifact_and_selected_plans as validate_fused_program_entry_establishments;
