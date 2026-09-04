//! Optimizer module role: executable entrance. Emission is selected by program shape.
//! Recovery and optimization histories are independently replayed before this
//! entrance; they affect evidence identities, not the fragment construction algorithm.

mod manifest;
mod ordinary;
mod ordinary_function;
mod source_kind;
mod statistics;
mod structural_unit;

use super::{FunctionFragmentEmissionError, StagedOptimizedFunctionFragmentEmissionSource};

pub(super) fn compute(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<ordinary::Emission, FunctionFragmentEmissionError> {
    let selected = source.selected_plan();
    if selected.structural_unit_functions.is_empty() {
        ordinary::compute(source)
    } else {
        structural_unit::compute(source)
    }
}
