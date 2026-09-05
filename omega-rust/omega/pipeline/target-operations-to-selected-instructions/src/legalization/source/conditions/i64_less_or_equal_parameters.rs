//! Exact signed-I64 inclusive comparison entrance.

use super::integer_parameter_comparison::{self, Kind};
use super::*;

pub(super) fn derive<'a>(
    function: usize,
    target: &'a target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
) -> Result<DerivedCondition<'a>, LegalizationError> {
    integer_parameter_comparison::derive(
        Kind::I64LessOrEqual,
        function,
        target,
        abstracted,
        optimized,
    )
}
