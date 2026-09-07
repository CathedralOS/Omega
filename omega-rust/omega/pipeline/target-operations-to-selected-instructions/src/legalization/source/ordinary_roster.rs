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
        } else if kind == super::publication_input::OrdinaryInputKind::Leaf {
            let (_, control) = crate::legalization::scalar_leaf::control(target_function)
                .expect("classified leaf input");
            let abi = crate::legalization::scalar_leaf::validate_input(
                index,
                target.target,
                target_function,
                abstracted,
                optimized,
            )?;
            let target_operations::TargetIntegerControl::Return {
                psi_return_edge, ..
            } = &control
            else {
                unreachable!("leaf input");
            };
            let leaf = super::leaves::derive_leaf(
                index,
                *psi_return_edge,
                &control,
                &abstracted.operations,
                &optimized.blocks[0].nodes,
                abstracted,
                optimized,
                &unit.accepted_obligation_facts,
                [
                    legalized_operations::LegalizedTemporaryId(0),
                    legalized_operations::LegalizedTemporaryId(1),
                ],
                matches!(
                    target_function.operation,
                    TargetOperation::ReturnIntegerExpression { .. }
                ),
            )?;
            let provenance = target_operations::TerminalPsiProvenance {
                operations: super::leaves::source_operations(&leaf.value),
                edges: vec![leaf.return_edge],
            };
            if provenance != target_function.provenance {
                return Err(Error::SourceCustodyMismatch);
            }
            rosters
                .functions
                .push(legalized_operations::LegalizedFunction::Leaf(
                    legalized_operations::LegalizedScalarLeafFunction {
                        machine: target_function.machine,
                        attachment: target_function.attachment,
                        provenance,
                        entry_block: optimized.entry,
                        abi: abi.clone(),
                        leaf,
                    },
                ));
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
