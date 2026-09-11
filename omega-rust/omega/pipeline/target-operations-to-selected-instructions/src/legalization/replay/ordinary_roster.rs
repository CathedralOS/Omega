//! Independently replay every function in the current graph roster.

use super::*;

pub(super) fn replay_remaining(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed: &LegalizedOperationPlan,
) -> Result<(), LegalizationError> {
    for target_function in &target.functions {
        let abstract_matches = abstract_plan
            .functions
            .iter()
            .filter(|candidate| candidate.machine == target_function.machine)
            .collect::<Vec<_>>();
        let optimized_matches = unit
            .functions
            .iter()
            .filter(|candidate| candidate.machine == target_function.machine)
            .collect::<Vec<_>>();
        let ([abstracted], [optimized]) =
            (abstract_matches.as_slice(), optimized_matches.as_slice())
        else {
            return Err(Error::SourceCustodyMismatch);
        };
        let graphs = proposed
            .scalar_functions
            .iter()
            .filter(|candidate| candidate.machine == target_function.machine)
            .collect::<Vec<_>>();
        let [graph] = graphs.as_slice() else {
            return Err(Error::NonCanonicalLegalizedPlan);
        };
        super::scalar_graph::replay(
            target_function,
            abstracted,
            optimized,
            target,
            abstract_plan,
            unit,
            proposed,
            graph,
        )?;
    }
    Ok(())
}
