//! Admit control cycles only after exact independent ranked source replay.
use super::*;
pub(in crate::legalization) fn validate_unit_custody(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let mut admitted = Vec::new();
    for function in &target.functions {
        if !matches!(function.operation, TargetOperation::RankedU32Countdown(_)) {
            continue;
        }
        let abstract_matches = abstract_plan
            .functions
            .iter()
            .filter(|source| source.machine == function.machine)
            .collect::<Vec<_>>();
        let optimized_matches = unit
            .functions
            .iter()
            .filter(|source| source.machine == function.machine)
            .collect::<Vec<_>>();
        let ([abstracted], [optimized]) =
            (abstract_matches.as_slice(), optimized_matches.as_slice())
        else {
            return Err(invalid);
        };
        // This is the input-only rank/proof/ABI and complete current graph join,
        // not a producer or a self-issued cycle assertion.
        super::match_input(function, abstracted, optimized, target, abstract_plan, unit)?;
        if admitted.contains(&function.machine) {
            return Err(invalid);
        }
        admitted.push(function.machine);
    }
    optimization_unit_semantics::validate_psi_optimization_unit_with_admitted_cycle_machines(
        unit, &admitted,
    )
    .map_err(|_| invalid)
}
