//! Optimizer module role: executable entrance. Emission is selected by program shape.
//! Recovery and optimization histories are independently replayed before this
//! entrance; they affect evidence identities, not the fragment construction algorithm.

mod manifest;

use super::projection::emit_resolved_function_fragments;
use super::{
    FunctionFragmentEmissionError, StagedOptimizedFunctionFragmentEmissionSource,
    ValidatedFunctionFragmentEmissionManifest,
};

pub(super) type Emission = (
    post_allocation_machine_to_selected_form_encoding::machine_code::FunctionFragmentEmissionPlan,
    ValidatedFunctionFragmentEmissionManifest,
);

pub(super) fn compute(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<Emission, FunctionFragmentEmissionError> {
    let fragments = emit_resolved_function_fragments(source.program())?;
    let source_manifest = source.function_relative_manifest().record();
    if source.post_allocation_manifest().record().selected != fragments.selected
        || source_manifest.selected != fragments.selected
        || source_manifest.resolved_layout != source.program().layout.identity
    {
        return Err(FunctionFragmentEmissionError::RootMismatch);
    }
    manifest::seal(fragments, source_manifest)
}
