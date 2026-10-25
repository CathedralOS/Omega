//! Executable entry admission before physical construction.

use diagnostics::Diagnostic;
use terminal_psi_to_abstract_operations::{
    NativeProgramEntrySettlement, TerminalPlacedViewEstablishment,
    ValidatedNativeProgramEntrySettlement, VerifiedNativeArtifactInput,
    validate_native_program_entry_settlement,
};

/// Admit the entry boundary required by an executable image. The complete
/// native input keeps the lowered entry, verified Terminal declaration, and
/// placed-view loans together; none can be supplied as an unrelated sidecar.
///
/// A source receiver erased from the executable still needs initialization
/// and cleanup custody. A retained receiver also needs a supported target
/// bridge. A semantic storage entry retains its physical bridge obligation
/// even without a receiver. Success permits construction, not publication: the emitted object
/// and image must independently replay the returned settlement.
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
    let selections = terminal_installation::admit_target_machine_selections(
        artifact.view(),
        source_profile.target_name(),
    );
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
        || terminal.entry != terminal_psi::TargetMachineCallable::Machine(plan.entry)
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
    let placed_view_establishments = input.placed_view_establishments();
    // The provider establishments bound into the input are loans the emitted
    // entry boundary must carry for the invocation's duration. Only the
    // hosted receiver bridge is such a boundary: an admission that retains
    // none cannot lend the referents and fails closed rather than publishing
    // an entry that observes custody nobody supplied.
    // Settlement retains the entry declaration, not an installed receiver.
    // This admission serves executable images; callable lowering and
    // explicit semantic wrappers retain their own boundaries.
    let lowered_entry = plan.function(plan.entry);
    let terminal_entry = terminal.machine(selected_entry);
    let lowered_attachment = lowered_entry.and_then(|function| function.attachment);
    let terminal_attachment = terminal_entry.and_then(|machine| machine.attachment);
    // Preserve declarations verbatim; an ordinary context has no attachment
    // authority. Checked settlement below identifies its exact retained place.
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
    // Semantic root delivery is an entry obligation, not receiver storage.
    // A free UEFI continuation still needs the physical-to-semantic bridge;
    // dropping its settlement would expose Extent inputs to firmware's ABI.
    let requires_semantic_bridge = program_entry.source().target_slot().schema
        == target::ProgramEntrySchema::ProgramStorageApplication;
    let requires_installed_storage = !input.private_entry_ram().is_empty();
    if !requires_installed_storage && !requires_semantic_bridge {
        return entry_boundary_without_receiver(placed_view_establishments);
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
    // Callback occupancy is orthogonal to receiver provisioning: each
    // materialization binds at its own registrar call site inside function
    // fragments, and the emitted object plus final image replay join them to
    // their private functions regardless of which entry boundary the program
    // selected.
    // This only permits physical construction to begin. The validated
    // settlement is the demand the object binder must satisfy exactly — the
    // bound placed-view establishments ride on it so the emitted entry
    // boundary is what must lend each referent — and the final image replay
    // must still prove the disjoint backing, exact entry pointer, stack
    // switch, and normal-return continuation before publication.
    Ok(Some(
        settled
            .with_placed_view_establishments(placed_view_establishments)
            .with_private_entry_ram(input.private_entry_ram()),
    ))
}

/// An admission that retains no hosted receiver boundary emits no shim that
/// can lend a placed-view referent; bound establishments reaching it reject
/// rather than publishing an entry whose custody was never supplied.
fn entry_boundary_without_receiver(
    placed_view_establishments: &[TerminalPlacedViewEstablishment],
) -> Result<Option<ValidatedNativeProgramEntrySettlement>, Vec<Diagnostic>> {
    if placed_view_establishments.is_empty() {
        return Ok(None);
    }
    Err(crate::native_artifact_error(
        "ProgramEntry placed-view custody",
        "bound placed-view establishments require the hosted entry boundary that lends each referent; this admission retains none",
    ))
}
