use super::super::shared::*;

pub(super) fn validate_callee(
    function: usize,
    callee: semantic_vocabulary::MachineId,
    call_plan: &calling_conventions::CallPlan,
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let mut targets = target
        .functions
        .iter()
        .enumerate()
        .filter(|(_, value)| value.machine == callee);
    let Some((_callee_index, target_callee)) = targets.next() else {
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
    if !crate::legalization::scalar_call_contract::accepts(
        target.target,
        target_callee,
        abstract_callee,
        optimized_callee,
        call_plan,
    ) {
        return Err(Error::UnsupportedSourceShape { function });
    }
    Ok(())
}
