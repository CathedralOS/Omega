//! Independent signed-I64 inclusive-comparison replay entrance.

use super::integer_parameter_comparison::{self, Kind};
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn replay<'a>(
    function: usize,
    architecture: target::Architecture,
    target: &'a target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    proposed_source: ValueId,
    proposed: &LegalizedCondition,
) -> Result<ReplayedCondition<'a>, LegalizationError> {
    integer_parameter_comparison::replay(
        Kind::I64LessOrEqual,
        function,
        architecture,
        target,
        abstracted,
        optimized,
        proposed_source,
        proposed,
    )
}
