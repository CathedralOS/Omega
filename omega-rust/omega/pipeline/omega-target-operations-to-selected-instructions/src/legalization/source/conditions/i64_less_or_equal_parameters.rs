//! Exact signed-I64 inclusive comparison entrance.

use super::integer_parameter_comparison::{self, Kind};
use super::*;

pub(super) fn derive<'a>(
    function: usize,
    target: &'a omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
) -> Result<DerivedCondition<'a>, LegalizationError> {
    integer_parameter_comparison::derive(
        Kind::I64LessOrEqual,
        function,
        target,
        abstracted,
        optimized,
    )
}
