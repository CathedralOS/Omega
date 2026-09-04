use omega_machine_code::FunctionFragmentEmissionPlan;
use omega_optimization_core::FunctionFragmentEmissionIdentity;

use super::super::{
    FunctionFragmentEmissionError, StagedOptimizedFunctionFragmentEmissionSource,
    ValidatedFunctionFragmentEmissionManifest,
};
use super::{manifest, ordinary_function, source_kind};

pub(super) type Emission = (
    FunctionFragmentEmissionPlan,
    ValidatedFunctionFragmentEmissionManifest,
);

pub(super) fn compute(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<Emission, FunctionFragmentEmissionError> {
    let selected_plan = source.selected_plan();
    // Both emission and replay validate the source before entering compute;
    // that validation joins this identity to the retained selected program.
    let selected_identity = source.post_allocation_manifest().record().selected;
    let layout = source.resolved_layout();
    let source_manifest = source.function_relative_manifest().record();
    if selected_identity != layout.selected()
        || selected_plan.target != layout.target()
        || selected_plan.functions.len() != layout.functions().len()
        || !selected_plan.structural_unit_functions.is_empty()
        || !layout.structural_unit_functions().is_empty()
        || source_manifest.selected != selected_identity
        || source_manifest.resolved_layout != layout.identity()
    {
        return Err(FunctionFragmentEmissionError::RootMismatch);
    }

    let mut functions = Vec::with_capacity(selected_plan.functions.len());
    for selected_function in &selected_plan.functions {
        let resolved = layout
            .functions()
            .iter()
            .find(|function| function.machine == selected_function.machine)
            .ok_or(FunctionFragmentEmissionError::MissingFunction(
                selected_function.machine,
            ))?;
        functions.push(ordinary_function::emit(selected_function, resolved)?);
    }
    let mut fragments = FunctionFragmentEmissionPlan {
        identity: FunctionFragmentEmissionIdentity::from_canonical_bytes(b"pending"),
        psi: selected_plan.psi,
        fuel_schedule: selected_plan.fuel_schedule,
        selected: selected_identity,
        target: selected_plan.target,
        entry: selected_plan.entry,
        functions,
        structural_unit_functions: Vec::new(),
    };
    fragments.identity = fragments.recomputed_identity();
    manifest::seal_ordinary(fragments, source_manifest, source_kind::of(source))
}
