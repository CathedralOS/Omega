//! Independent replay of ordered I64 strict-less-than custody.

use super::integer_parameter_comparison::{self, Kind};
use super::{LegalizationError, LegalizedCondition, ReplayedCondition, ValueId};

pub(super) fn replay<'a>(
    function: usize,
    architecture: omega_target::Architecture,
    target: &'a omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    proposed_source: ValueId,
    proposed: &LegalizedCondition,
) -> Result<ReplayedCondition<'a>, LegalizationError> {
    integer_parameter_comparison::replay(
        Kind::I64LessThan,
        function,
        architecture,
        target,
        abstracted,
        optimized,
        proposed_source,
        proposed,
    )
}
