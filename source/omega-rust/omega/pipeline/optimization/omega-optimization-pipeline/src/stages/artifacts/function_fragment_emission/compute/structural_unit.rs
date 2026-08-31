mod call;
mod function;

use omega_machine_code::FunctionFragmentEmissionPlan;
use omega_optimization_core::FunctionFragmentEmissionIdentity;

use crate::StagedOptimizedStructuralUnitFunctionRelativeRealization;

use super::super::{FunctionFragmentEmissionError, StagedOptimizedFunctionFragmentEmissionSource};
use super::manifest;
use super::ordinary::Emission;

pub(super) fn compute(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
    realization: &StagedOptimizedStructuralUnitFunctionRelativeRealization,
) -> Result<Emission, FunctionFragmentEmissionError> {
    let selected_plan = source.selected_plan();
    let layout = realization.layout();
    let source_manifest = realization.manifest().record();
    if !selected_plan.functions.is_empty()
        || !layout.functions().is_empty()
        || selected_plan.structural_unit_functions.len() != layout.structural_unit_functions().len()
        || selected_plan.structural_unit_functions.is_empty()
        || selected_plan.target != layout.target()
        || source_manifest.selected != layout.selected()
        || source_manifest.resolved_layout != layout.identity()
    {
        return Err(FunctionFragmentEmissionError::RootMismatch);
    }

    let mut structural_unit_functions =
        Vec::with_capacity(selected_plan.structural_unit_functions.len());
    for (selected, resolved) in selected_plan
        .structural_unit_functions
        .iter()
        .zip(layout.structural_unit_functions())
    {
        structural_unit_functions.push(function::emit(selected, resolved)?);
    }
    let mut fragments = FunctionFragmentEmissionPlan {
        identity: FunctionFragmentEmissionIdentity::from_canonical_bytes(b"pending"),
        psi: selected_plan.psi,
        fuel_schedule: selected_plan.fuel_schedule,
        selected: source_manifest.selected,
        target: selected_plan.target,
        entry: selected_plan.entry,
        functions: Vec::new(),
        structural_unit_functions,
    };
    fragments.identity = fragments.recomputed_identity();
    manifest::seal_structural(fragments, source_manifest)
}
