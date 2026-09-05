//! Optimizer module role: executable entrance. Emission is selected by program shape.
//! Recovery and optimization histories are independently replayed before this
//! entrance; they affect evidence identities, not the fragment construction algorithm.

mod manifest;

use super::{
    FunctionFragmentEmissionError, FunctionFragmentEmissionSourceKind,
    StagedOptimizedFunctionFragmentEmissionSource, ValidatedFunctionFragmentEmissionManifest,
};

pub(super) type Emission = (
    omega_machine_code::FunctionFragmentEmissionPlan,
    ValidatedFunctionFragmentEmissionManifest,
);

pub(super) fn compute(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<Emission, FunctionFragmentEmissionError> {
    let fragments = omega_machine_emission::emit_resolved_function_fragments(source.program())?;
    let source_manifest = source.function_relative_manifest().record();
    if source.post_allocation_manifest().record().selected != fragments.selected
        || source_manifest.selected != fragments.selected
        || source_manifest.resolved_layout != source.program().layout.identity
    {
        return Err(FunctionFragmentEmissionError::RootMismatch);
    }
    let source_kind = if source.selected_plan().structural_unit_functions.is_empty() {
        source.source_kind()
    } else {
        FunctionFragmentEmissionSourceKind::StructuralUnitV1
    };
    manifest::seal(fragments, source_manifest, source_kind)
}
