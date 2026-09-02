//! Exact ordered U64 entry-parameter inclusive comparison selection.

use super::integer_parameter_comparison::{self, Kind};
use super::{DerivedCondition, LegalizationError};

pub(super) fn derive<'a>(
    function: usize,
    target: &'a omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
) -> Result<DerivedCondition<'a>, LegalizationError> {
    integer_parameter_comparison::derive(Kind::LessOrEqual, function, target, abstracted, optimized)
}
