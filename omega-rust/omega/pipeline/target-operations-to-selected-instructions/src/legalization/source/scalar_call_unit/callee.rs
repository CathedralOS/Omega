use super::super::functions::derive_source_function;
use super::super::shared::*;

pub(super) fn validate_callee(
    function: usize,
    callee: semantic_vocabulary::MachineId,
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let mut targets = target
        .functions
        .iter()
        .enumerate()
        .filter(|(_, value)| value.machine == callee);
    let Some((callee_index, target_callee)) = targets.next() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if targets.next().is_some() {
        return Err(Error::SourceCustodyMismatch);
    }
    let mut abstracts = abstract_plan
        .functions
        .iter()
        .filter(|value| value.machine == callee);
    let mut optimized = unit
        .functions
        .iter()
        .filter(|value| value.machine == callee);
    let (Some(abstract_callee), Some(optimized_callee)) = (abstracts.next(), optimized.next())
    else {
        return Err(Error::SourceCustodyMismatch);
    };
    if abstracts.next().is_some() || optimized.next().is_some() {
        return Err(Error::SourceCustodyMismatch);
    }
    let legalized = derive_source_function(
        callee_index,
        target_callee,
        abstract_callee,
        optimized_callee,
        &unit.accepted_obligation_facts,
    )?;
    if legalized.recipe
        != legalized_operations::LegalizationRecipe::ReturnU64IntegerEqualParametersConditionalV1
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    Ok(())
}
