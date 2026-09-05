//! Independent exact U64 parameter inequality replay entrance.

use super::integer_parameter_not_equal;
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
    integer_parameter_not_equal::replay(
        function,
        architecture,
        target,
        abstracted,
        optimized,
        proposed_source,
        proposed,
    )
}
