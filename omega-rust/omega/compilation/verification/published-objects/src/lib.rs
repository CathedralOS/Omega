//! Recheck the emitted object against original stage inputs on an explicit
//! audit request. Only a compile that requested an audit retains those
//! inputs; without them each audit refuses rather than passing.

#![forbid(unsafe_code)]

use image_emission::{FragmentReplay, ObjectArtifact, RetainedTargetOperations};

/// Compare the current fixed-frame output with retained original allocation,
/// encoding and layout inputs, and its allocation-recovery selection with the
/// one physical planning used. Missing originals refuse this explicit audit.
pub fn verify_retained_fixed_frame(
    artifact: &ObjectArtifact,
) -> Result<(), diagnostics::Diagnostic> {
    for private in artifact.private_functions() {
        verify_retained_fixed_frame(&private.body)?;
    }
    let replay = artifact.fragment_replay().ok_or_else(|| {
        diagnostics::Diagnostic::error(
            "fixed-frame audit requires retained originals (ReplayUnavailable)",
        )
    })?;
    verify_physical_origins(artifact, replay)?;
    let current = replay.fragment_source().source().source().source().source();
    current
        .verify_original_correspondence()
        .map_err(|error| diagnostics::Diagnostic::error(error.to_string()))?;
    let original = retained_target(replay, "fixed-frame")?;
    let expected = original
        .target_for_correspondence()
        .optimized()
        .selections()
        .for_phase(optimization_core::OptimizationExecutionPhase::AllocationRecovery)
        .identity();
    if current
        .function_relative_manifest()
        .record()
        .allocation_recovery_selections
        != expected
    {
        return Err(diagnostics::Diagnostic::error(
            "fixed-frame allocation recovery differs from the selection physical planning used",
        ));
    }
    Ok(())
}

/// Reconstruct the physical instruction projections while the explicit audit owns its originals.
fn verify_physical_origins(
    artifact: &ObjectArtifact,
    replay: &FragmentReplay,
) -> Result<(), diagnostics::Diagnostic> {
    let source = replay.fragment_source();
    let current = source.source().source().source().source();
    let program = current.original_physical_program().ok_or_else(|| {
        diagnostics::Diagnostic::error(
            "original physical instructions were not retained (ReplayUnavailable)",
        )
    })?;
    let facts = source.emission_facts();
    let mismatch = diagnostics::Diagnostic::error;
    let calls = physical_planning::call_origins::capture(program);
    if calls != facts.calls || &calls != artifact.call_origins().as_ref() {
        return Err(mismatch(
            "object call origins differ from retained physical instructions",
        ));
    }
    let foreign = physical_planning::foreign_call_plans::capture(program, current.frame_layout());
    if foreign != facts.foreign_calls
        || foreign.as_ref().ok() != Some(artifact.foreign_call_plans().as_ref())
    {
        return Err(mismatch(
            "object foreign call plans differ from retained physical instructions",
        ));
    }
    if physical_planning::image_origins::capture(program.functions.iter()) != facts.image_origins {
        return Err(mismatch(
            "image address origins differ from retained physical instructions",
        ));
    }
    if physical_planning::physical_facts::port_writes(program) != facts.port_writes {
        return Err(mismatch(
            "port event origins differ from retained physical instructions",
        ));
    }
    if physical_planning::boundary_settlements::capture(program) != facts.boundary_settlements {
        return Err(mismatch(
            "boundary settlements differ from retained physical instructions",
        ));
    }
    if physical_planning::internal_call_plans::capture(
        program,
        current.frame_layout(),
        &facts.program,
    ) != facts.internal_calls
    {
        return Err(mismatch(
            "internal call plans differ from retained physical instructions",
        ));
    }
    if installation_admission::selected::cpu_origins::capture(program) != facts.cpu {
        return Err(mismatch(
            "CPU origins differ from retained physical instructions",
        ));
    }
    if installation_admission::selected::fault_origins::capture(program) != facts.faults {
        return Err(mismatch(
            "fault origins differ from retained physical instructions",
        ));
    }
    Ok(())
}

/// Replay each body's retained Omega 04 rewrite run against the record its
/// allocated program carries. Returns whether any body had a run to check;
/// `false` means no selected-instruction rewrite phase produced this artifact.
pub fn verify_retained_selected_optimization(
    artifact: &ObjectArtifact,
) -> Result<bool, diagnostics::Diagnostic> {
    let mut checked = false;
    for private in artifact.private_functions() {
        checked |= verify_retained_selected_optimization(&private.body)?;
    }
    let replay = artifact.fragment_replay().ok_or_else(|| {
        diagnostics::Diagnostic::error(
            "selected-instruction rewrite audit requires retained originals (ReplayUnavailable)",
        )
    })?;
    let ran = replay
        .fragment_source()
        .source()
        .source()
        .source()
        .source()
        .verify_selected_optimization_replay()
        .map_err(|error| diagnostics::Diagnostic::error(error.to_string()))?;
    Ok(checked || ran)
}

pub fn verify_retained_target_operations(
    artifact: &ObjectArtifact,
) -> Result<(), diagnostics::Diagnostic> {
    for private in artifact.private_functions() {
        verify_retained_target_operations(&private.body)?;
    }
    let replay = artifact.fragment_replay().ok_or_else(|| {
        diagnostics::Diagnostic::error("target-operation audit requires retained target operations")
    })?;
    let original = retained_target(replay, "target-operation")?;
    let current = original.target_for_correspondence();
    let optimized = current.optimized();
    // Every access reference native program planning established must land on backing
    // the selected program actually allocates. Ordinary emission builds
    // from the current program, so this check runs here and in the object
    // replay, not on the default path.
    replay
        .verify_access_geometry()
        .map_err(|error| diagnostics::Diagnostic::error(error.to_string()))?;
    // The validator judges the plan published for this object,
    // not a plan prepared again here.
    let target = current.working_program_for_validation();
    let callable_roots = current
        .lowering_facts()
        .callable_invocation_roots(target)
        .map_err(diagnostics::Diagnostic::error)?;
    native_program_planning::replay::verify_execution_plan(
        current.execution_plan(),
        native_program_planning::replay::ExecutionPlanReplayInputs {
            target,
            abstract_plan: optimized.plan(),
            unit: optimized.unit(),
            origins: current.boundary_origins(),
            installed_interrupts: current.lowering_facts().installed_interrupts(),
            closed_module: current.lowering_facts().closed_module(),
            callable_roots: &callable_roots,
            pre_physical: optimized.pre_physical_manifest().identity,
        },
    )
    .map_err(|error| diagnostics::Diagnostic::error(error.to_string()))?;
    let expected = native_program_planning::replay::capture_runtime_borrowed_storage_layouts(
        optimized,
        current.lowering_facts(),
        current.target(),
    )
    .map_err(|error| {
        diagnostics::Diagnostic::error(format!("borrowed storage geometry audit failed: {error:?}"))
    })?;
    if expected != current.execution_plan().plan().borrowed_storage_layouts {
        return Err(diagnostics::Diagnostic::error(
            "published borrowed storage geometry differs from admitted declarations",
        ));
    }
    Ok(())
}

/// The native planning output retained beside this object, joined to it: the current
/// allocation must name the pre-physical manifest that output carried.
fn retained_target<'replay>(
    replay: &'replay FragmentReplay,
    audit: &str,
) -> Result<&'replay RetainedTargetOperations, diagnostics::Diagnostic> {
    let original = replay.target_source().ok_or_else(|| {
        diagnostics::Diagnostic::error(format!("{audit} audit requires retained target operations"))
    })?;
    let current = replay.fragment_source().source().source().source().source();
    if !original.belongs_to(replay.fragment_source().emission_facts())
        || current.post_allocation_manifest().record().pre_physical
            != original
                .target_for_correspondence()
                .optimized()
                .pre_physical_manifest()
                .identity
    {
        return Err(diagnostics::Diagnostic::error(format!(
            "{audit} audit: retained target operations name another pre-physical manifest"
        )));
    }
    Ok(original)
}
