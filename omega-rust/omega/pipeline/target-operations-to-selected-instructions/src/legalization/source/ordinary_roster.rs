//! Existing per-function catalog routing outside an atomic plan family.

use super::*;

pub(super) fn derive_remaining(
    rosters: &mut SourceFunctionRosters,
    projected: Option<&legalized_operations::LegalizedProjectedStructuralCallReturn>,
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    for (index, target_function) in target.functions.iter().enumerate() {
        if projected.is_some_and(|closure| {
            target_function.machine == closure.caller.machine
                || target_function.machine == closure.callee.machine
        }) {
            continue;
        }
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
        if matches!(target_function.operation, TargetOperation::UnitBody(_)) {
            if match_scalar_call_unit_form(target_function).is_some() {
                rosters
                    .scalar_call_unit_functions
                    .push(derive_source_scalar_call_unit_function(
                        index,
                        target_function,
                        abstracted,
                        optimized,
                        target,
                        abstract_plan,
                        unit,
                    )?);
            } else if let Some(form) = match_unit_form(target_function, abstracted, optimized) {
                rosters.unit_functions.push(derive_source_unit_function(
                    index,
                    target_function,
                    abstracted,
                    optimized,
                    form,
                )?);
            } else {
                let matched = match_structural_unit_form(target_function, abstracted, optimized)
                    .ok_or(Error::UnsupportedSourceShape { function: index })?;
                rosters
                    .structural_unit_functions
                    .push(derive_source_structural_unit_function(
                        index,
                        target_function,
                        abstracted,
                        optimized,
                        target,
                        abstract_plan,
                        unit,
                        matched,
                    )?);
            }
        } else {
            rosters.functions.push(derive_source_function(
                index,
                target_function,
                abstracted,
                optimized,
                &unit.accepted_obligation_facts,
            )?);
        }
    }
    Ok(())
}
