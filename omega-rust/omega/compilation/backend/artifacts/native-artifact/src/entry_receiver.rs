/*
We admit the executable entry boundary (`admit_executable_entry`) before
physical object construction begins. We verify that the source-selected entry
deployment owns the target, that lowered attachment and structural parameter
declarations match the canonical Terminal entry, and that any bound
access-view establishments or private entry RAM recipes are accompanied by a
hosted receiver or physical entry settlement.
*/

use diagnostics::Diagnostic;
use terminal_installation::admit_target_machine_selections;
use terminal_psi_to_abstract_operations::{
    NativeProgramEntrySettlement, TerminalAccessViewEstablishment,
    ValidatedNativeProgramEntrySettlement, VerifiedNativeArtifactInput,
    validate_native_program_entry_settlement,
};

pub fn admit_executable_entry(
    input: &VerifiedNativeArtifactInput,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    program_entry: NativeProgramEntrySettlement<'_>,
    native_target: target::NativeTarget,
) -> Result<Option<ValidatedNativeProgramEntrySettlement>, Vec<Diagnostic>> {
    let plan = input.plan();
    let terminal = input.context().module();
    let source_profile = program_entry.source().target_slot().owner;
    if source_profile.native_realization() != Some(native_target) {
        return Err(crate::native_artifact_error(
            "ProgramEntry installed arguments",
            "source-selected entry deployment does not own the native target",
        ));
    }
    let selections = admit_target_machine_selections(artifact.view(), source_profile.target_name());
    let selected_entry = selections
        .entry(
            artifact.view().identity(),
            terminal,
            terminal_psi::TargetMachineCallable::Machine(plan.entry),
        )
        .map_err(|error| crate::native_artifact_error("ProgramEntry installed arguments", error))?;
    if selected_entry != plan.entry
        || plan.psi != artifact.view().identity()
        || terminal != artifact.semantic_module()
        || input
            .optimization_input()
            .target_machine_selections()
            .is_some_and(|retained| retained != &selections)
        || terminal.entry != Some(terminal_psi::TargetMachineCallable::Machine(plan.entry))
            && (input
                .optimization_input()
                .target_machine_selections()
                .is_none()
                || program_entry.checked_entry.is_none())
        || program_entry.checked_entry.is_some_and(|receipt| {
            program_entry
                .physical_adapter()
                .map_or(receipt.terminal_entry(), |adapter| adapter.machine())
                != selected_entry
                || receipt.source_signature_identity() != program_entry.source().identity().bytes()
                || receipt.terminal_psi_identity() != artifact.view().identity()
        })
    {
        return Err(crate::native_artifact_error(
            "ProgramEntry installed arguments",
            "native entry lost its exact source-selected Terminal member",
        ));
    }
    let access_view_establishments = input.access_view_establishments();
    let lowered_entry = plan.function(plan.entry);
    let terminal_entry = terminal.machine(selected_entry);
    let lowered_attachment = lowered_entry.and_then(|function| function.attachment);
    let terminal_attachment = terminal_entry.and_then(|machine| machine.attachment);
    let parameters_preserved = match (lowered_entry, terminal_entry) {
        (Some(function), Some(machine)) => {
            function.structural_parameters == machine.structural_parameters
        }
        _ => false,
    };
    if lowered_attachment != terminal_attachment || !parameters_preserved {
        return Err(crate::native_artifact_error(
            "ProgramEntry installed arguments",
            "lowered entry does not preserve the source-selected receiver identity",
        ));
    }
    let requires_semantic_bridge = program_entry.source().target_slot().schema
        == target::ProgramEntrySchema::ProgramStorageApplication;
    let requires_installed_storage = !input.private_entry_ram().is_empty();
    if !requires_installed_storage && !requires_semantic_bridge {
        return entry_boundary_without_receiver(access_view_establishments);
    }
    let checked_entry = program_entry.checked_entry.ok_or_else(|| {
        crate::native_artifact_error(
            "ProgramEntry installed arguments",
            "executable entry bridge requires exact checked entry custody",
        )
    })?;
    let settled = validate_native_program_entry_settlement(
        artifact,
        checked_entry,
        program_entry,
        native_target,
    )
    .map_err(|error| crate::native_artifact_error("ProgramEntry receiver custody", error))?;
    Ok(Some(
        settled
            .with_access_view_establishments(access_view_establishments)
            .with_private_entry_ram(input.private_entry_ram()),
    ))
}

fn entry_boundary_without_receiver(
    access_view_establishments: &[TerminalAccessViewEstablishment],
) -> Result<Option<ValidatedNativeProgramEntrySettlement>, Vec<Diagnostic>> {
    if access_view_establishments.is_empty() {
        return Ok(None);
    }
    Err(crate::native_artifact_error(
        "ProgramEntry access-view custody",
        "bound access-view establishments require the hosted entry boundary that lends each referent; this admission retains none",
    ))
}
