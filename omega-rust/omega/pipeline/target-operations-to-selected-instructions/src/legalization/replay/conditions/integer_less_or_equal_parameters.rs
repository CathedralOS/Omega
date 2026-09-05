//! Independent selection of ordered U64 inclusive comparison replay.

use super::integer_parameter_comparison::{self, Kind};
use super::{LegalizationError, LegalizedCondition, ReplayedCondition, ValueId};

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
        Kind::LessOrEqual,
        function,
        architecture,
        target,
        abstracted,
        optimized,
        proposed_source,
        proposed,
    )
}
