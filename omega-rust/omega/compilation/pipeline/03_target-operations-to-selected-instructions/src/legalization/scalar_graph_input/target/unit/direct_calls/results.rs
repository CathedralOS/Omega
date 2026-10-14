//! Reconstruct result identity, storage and returned custody independently of
//! argument placement. Reference leaves resolve against the state before the
//! call, so a producer cannot substitute a new home for the caller's referent.

use abstract_operations::{
    AbstractOperation, AbstractOperationPlan, AbstractResult, AbstractStructuralCallResult,
};
use calling_conventions::CallPlan;
use optimization_unit::PsiOptimizationFunction;
use target_operations::{TargetCallResult, TargetUnitScalarHomeRequirement};

use crate::LegalizationError;
use crate::legalization::scalar_graph_input::{aggregate_results, reference_custody, scalar_shape};

pub(super) fn validate(
    retained: &TargetCallResult,
    source: &AbstractOperation,
    callee: Option<&PsiOptimizationFunction>,
    call_plan: &CallPlan,
    custody: &reference_custody::Custody,
    caller: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
) -> Result<(), LegalizationError> {
    match (retained, source) {
        (
            TargetCallResult::Unit,
            AbstractOperation::CallStructural {
                result: AbstractStructuralCallResult::Unit,
                ..
            },
        ) => {
            if call_plan.result.is_some() {
                return Err(LegalizationError::custody());
            }
        }
        (
            TargetCallResult::Scalar(home),
            AbstractOperation::Call {
                psi_operation,
                result,
                scalar_type,
                ..
            },
        ) => scalar(
            *home,
            *psi_operation,
            AbstractResult {
                value: *result,
                scalar_type: *scalar_type,
            },
            call_plan,
        )?,
        (
            TargetCallResult::Scalar(home),
            AbstractOperation::CallStructural {
                psi_operation,
                result: AbstractStructuralCallResult::Scalar(result),
                ..
            },
        ) => scalar(*home, *psi_operation, *result, call_plan)?,
        (
            TargetCallResult::Structural {
                result,
                callee_result,
                result_home,
                reference_results,
            },
            AbstractOperation::CallStructural {
                psi_operation,
                callee: target,
                result:
                    AbstractStructuralCallResult::Structural {
                        result: expected_result,
                        ..
                    },
                structural_arguments,
                requirement_obligations: _,
                ..
            },
        ) => {
            let expected_home = Some(aggregate_results::result_home(
                caller,
                expected_result.place,
                plan,
            )?);
            let expected_references = match (callee_result, callee, target) {
                (
                    target_operations::TargetStructuralResultContract::Machine(declaration),
                    Some(callee),
                    abstract_operations::AbstractCallTarget::Machine(machine),
                ) => {
                    if callee.machine != *machine
                        || callee.result.structural() != Some(declaration)
                        || result.structural_type != declaration.structural_type
                        || result.multiplicity != declaration.multiplicity
                    {
                        return Err(LegalizationError::custody());
                    }
                    reference_custody::reference_results(
                        caller,
                        Some(callee),
                        terminal_semantics::ReferenceCallContract::Machine {
                            parameters: &callee.structural_parameters,
                            sources: &declaration.reference_sources,
                        },
                        structural_arguments,
                        expected_result,
                        *psi_operation,
                        custody,
                        &plan.structural_types,
                    )
                    .map_err(|error| error.attributed(caller.machine, Some(*psi_operation)))?
                }
                (
                    target_operations::TargetStructuralResultContract::Requirement(declaration),
                    None,
                    abstract_operations::AbstractCallTarget::Dynamic {
                        interface, slot, ..
                    },
                ) => {
                    let requirement = plan
                        .dynamic_dispatch
                        .interface(*interface)
                        .and_then(|interface| interface.slots.get(*slot as usize))
                        .ok_or(LegalizationError::custody())?;
                    if requirement.result
                        != terminal_psi::BoundaryMachineResult::Structural(declaration.clone())
                        || !requirement.matches_operation_result(
                            &terminal_psi::OperationResult::Structural(expected_result.clone()),
                        )
                    {
                        return Err(LegalizationError::custody());
                    }
                    reference_custody::reference_results(
                        caller,
                        None,
                        terminal_semantics::ReferenceCallContract::Requirement(requirement),
                        structural_arguments,
                        expected_result,
                        *psi_operation,
                        custody,
                        &plan.structural_types,
                    )
                    .map_err(|error| error.attributed(caller.machine, Some(*psi_operation)))?
                }
                _ => return Err(LegalizationError::custody()),
            };
            // Claims cross as custody, not bytes: the enclosing call check
            // replays the claim and requirement rosters exactly and both ride
            // the legalized instruction. Outcome-specific evidence is
            // proof-only; unit validation joined it before target lowering.
            if result != expected_result
                || *result_home != expected_home
                || *reference_results != expected_references
            {
                return Err(LegalizationError::custody());
            }
        }
        _ => return Err(LegalizationError::custody()),
    }
    Ok(())
}

fn scalar(
    retained: TargetUnitScalarHomeRequirement,
    operation: semantic_vocabulary::OperationId,
    result: AbstractResult,
    call_plan: &CallPlan,
) -> Result<(), LegalizationError> {
    let shape = scalar_shape(result.scalar_type).ok_or_else(|| LegalizationError::custody())?;
    let expected = TargetUnitScalarHomeRequirement {
        defining_operation: operation,
        source_value: result.value,
        scalar_type: result.scalar_type,
        shape,
    };
    if retained != expected
        || call_plan.result.as_ref().map(|placement| placement.shape) != Some(shape)
    {
        return Err(LegalizationError::custody());
    }
    Ok(())
}
