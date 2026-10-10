use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::super::DeadCompareError;
use super::{EquivalentCompareError, ValidatedEquivalentCompare, admission};
use crate::ValidatedSelectedAnalysis;

/// Remove one admitted compare whose published condition-state units the
/// earlier value-equivalent shadows already publish with identical values.
/// The instruction leaves the plan, boundary settlements shift over the
/// removed ordinal, and every other function, block, instruction, register,
/// call, settlement, and access is retained — replay independently confirms
/// exactly that.
pub(crate) fn remove_equivalent_compare(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedEquivalentCompare, EquivalentCompareError> {
    super::super::admission::remove_compare(
        source,
        function_index,
        compare,
        environment,
        budget,
        admission::admit,
        |error| match error {
            DeadCompareError::IdentityOverflow => EquivalentCompareError::IdentityOverflow,
            _ => EquivalentCompareError::SourceMismatch,
        },
        super::validation::seal_equivalent_compare,
    )
}
