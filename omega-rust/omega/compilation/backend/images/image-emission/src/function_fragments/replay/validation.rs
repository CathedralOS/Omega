//! Recheck stage 02 against the retained program on an explicit audit request.

use crate::function_fragments::source;

/// Compare the current fixed-frame output with retained original allocation,
/// encoding and layout inputs. Missing originals refuse this explicit audit.
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
    replay
        .0
        .source()
        .source()
        .source()
        .source()
        .verify_original_correspondence()
        .map_err(|error| diagnostics::Diagnostic::error(error.to_string()))
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
    let current = replay
        .0
        .source()
        .source()
        .source()
        .source()
        .optimized_target();
    let optimized = current.optimized();
    // Every access reference stage 02 established must land on backing
    // the selected program actually allocates. Ordinary emission builds
    // from the prepared plan, so this check runs here and in the object
    // replay, not on the default path.
    source::retained_source_validation::establish_geometry(&replay.0)
        .map_err(|error| diagnostics::Diagnostic::error(error.to_string()))?;
    // The validator judges the plan stage 02 published for this object,
    // not a plan prepared again here.
    let target = crate::retained_working_program::target_operations(&replay.0);
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
