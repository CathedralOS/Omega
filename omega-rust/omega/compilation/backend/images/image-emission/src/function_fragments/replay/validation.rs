//! Recheck the emitted object against original stage inputs on an explicit
//! audit request. Only a compile that requested an audit retains those
//! inputs; without them each audit refuses rather than passing.

use crate::function_fragments::source;

/// Compare the current fixed-frame output with retained original allocation,
/// encoding and layout inputs, and its allocation-recovery selection with the
/// one stage 01 ran. Missing originals refuse this explicit audit.
pub fn verify_retained_fixed_frame(
    artifact: &crate::ObjectArtifact,
) -> Result<(), diagnostics::Diagnostic> {
    for private in &artifact.private_functions {
        verify_retained_fixed_frame(&private.body)?;
    }
    let replay = artifact.fragment_replay.as_ref().ok_or_else(|| {
        diagnostics::Diagnostic::error(
            "fixed-frame audit requires retained current artifact custody",
        )
    })?;
    let current = replay.0.source().source().source().source();
    current
        .verify_original_correspondence()
        .map_err(|error| diagnostics::Diagnostic::error(error.to_string()))?;
    let original = retained_target(replay, "fixed-frame")?;
    let expected = original
        .target()
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
            "fixed-frame allocation recovery differs from the selection stage 01 ran",
        ));
    }
    Ok(())
}

/// Replay each body's retained Omega 04 rewrite run against the record its
/// allocated program carries. Returns whether any body had a run to check;
/// `false` means no selected-instruction rewrite phase produced this artifact.
pub fn verify_retained_selected_optimization(
    artifact: &crate::ObjectArtifact,
) -> Result<bool, diagnostics::Diagnostic> {
    let mut checked = false;
    for private in &artifact.private_functions {
        checked |= verify_retained_selected_optimization(&private.body)?;
    }
    let replay = artifact.fragment_replay.as_ref().ok_or_else(|| {
        diagnostics::Diagnostic::error(
            "selected-instruction rewrite audit requires retained pipeline input",
        )
    })?;
    let ran = replay
        .0
        .source()
        .source()
        .source()
        .source()
        .verify_selected_optimization_replay()
        .map_err(|error| diagnostics::Diagnostic::error(error.to_string()))?;
    Ok(checked || ran)
}

pub fn verify_retained_target_operations(
    artifact: &crate::ObjectArtifact,
) -> Result<(), diagnostics::Diagnostic> {
    for private in &artifact.private_functions {
        verify_retained_target_operations(&private.body)?;
    }
    let replay = artifact.fragment_replay.as_ref().ok_or_else(|| {
        diagnostics::Diagnostic::error("target-operation audit requires retained pipeline input")
    })?;
    let original = retained_target(replay, "target-operation")?;
    let current = original.target();
    let optimized = current.optimized();
    // Every access reference stage 02 established must land on backing
    // the selected program actually allocates. Ordinary emission builds
    // from the current program, so this check runs here and in the object
    // replay, not on the default path.
    source::retained_source_validation::establish_geometry(&replay.0, original)
        .map_err(|error| diagnostics::Diagnostic::error(error.to_string()))?;
    // The validator judges the plan stage 02 published for this object,
    // not a plan prepared again here.
    let target = crate::retained_working_program::target_operations(original);
    let callable_roots = current
        .lowering_facts()
        .callable_invocation_roots(target)
        .map_err(diagnostics::Diagnostic::error)?;
    native_preparation::verify_execution_plan(
        current.execution_plan(),
        native_preparation::ExecutionPlanReplayInputs {
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
    let expected =
        abstract_operations_to_target_operations::capture_runtime_borrowed_storage_layouts(
            optimized,
            current.lowering_facts(),
            current.target(),
        )
        .map_err(|error| {
            diagnostics::Diagnostic::error(format!(
                "borrowed storage geometry audit failed: {error:?}"
            ))
        })?;
    if expected != current.execution_plan().plan().borrowed_storage_layouts {
        return Err(diagnostics::Diagnostic::error(
            "published borrowed storage geometry differs from admitted declarations",
        ));
    }
    Ok(())
}

/// Stage 02's output retained beside this object, joined to it: the current
/// allocation must name the pre-physical manifest that output carried.
fn retained_target<'replay>(
    replay: &'replay super::FragmentReplay,
    audit: &str,
) -> Result<&'replay crate::RetainedTargetOperations, diagnostics::Diagnostic> {
    let original = replay.1.as_ref().ok_or_else(|| {
        diagnostics::Diagnostic::error(format!("{audit} audit requires retained target operations"))
    })?;
    let current = replay.0.source().source().source().source();
    if current.post_allocation_manifest().record().pre_physical
        != original
            .target()
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
