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
        let kind = super::publication_input::kind(target_function, abstracted);
        if crate::legalization::scalar_graph_input::match_input(
            target_function,
            abstracted,
            optimized,
            target,
            abstract_plan,
            unit,
        )
        .is_ok()
        {
            rosters.scalar_functions.push(super::scalar_graph::derive(
                target_function,
                abstracted,
                optimized,
                target,
                abstract_plan,
                unit,
            )?);
        } else if kind == super::publication_input::OrdinaryInputKind::Unit {
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
        } else if kind == super::publication_input::OrdinaryInputKind::SharedReturn {
            rosters.functions.push(
                legalized_operations::LegalizedFunction::SharedReturnConditional(
                    super::shared_return::derive(
                        index,
                        target.target,
                        target_function,
                        abstracted,
                        optimized,
                    )?,
                ),
            );
        } else {
            rosters
                .functions
                .push(legalized_operations::LegalizedFunction::Conditional(
                    derive_source_function(
                        index,
                        target_function,
                        abstracted,
                        optimized,
                        &unit.accepted_obligation_facts,
                    )?,
                ));
        }
    }
    Ok(())
}
