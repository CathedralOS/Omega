//! Reconstruct result identity, storage and returned custody independently of
//! argument placement. Reference leaves resolve against the state before the
//! call, so a producer cannot substitute a new home for the caller's referent.

use abstract_operations::{AbstractOperation, AbstractOperationPlan, AbstractResult};
use calling_conventions::CallPlan;
use optimization_unit::PsiOptimizationFunction;
use target_operations::{TargetCallResult, TargetUnitScalarHomeRequirement};

use crate::LegalizationError;
use crate::legalization::scalar_graph_input::{aggregate_results, reference_custody, scalar_shape};

pub(super) fn validate(
    retained: &TargetCallResult,
    source: &AbstractOperation,
    callee: &PsiOptimizationFunction,
    call_plan: &CallPlan,
    custody: &reference_custody::Custody,
    caller: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
) -> Result<(), LegalizationError> {
    let invalid = || LegalizationError::SourceCustodyMismatch;
    match (retained, source) {
        (TargetCallResult::Unit, AbstractOperation::CallUnit { .. }) => {
            if call_plan.result.is_some() {
                return Err(invalid());
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
            AbstractOperation::CallStructuralScalar {
                psi_operation,
                result,
                ..
            },
        ) => scalar(*home, *psi_operation, *result, call_plan)?,
        (
            TargetCallResult::Structural {
                result,
                callee_result,
                result_home,
                reference_results,
                returned_claim_transfers,
            },
            AbstractOperation::CallStructural {
                result: expected_result,
                structural_arguments,
                claim_transfers,
                returned_claim_transfers: expected_returns,
                requirement_obligations,
                crash_continuations,
                selected_evidence,
                ..
            },
        ) => {
            let reference_only = plan.structural_types.iter().any(|declaration| {
                declaration.id == expected_result.structural_type
                    && matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::Reference { .. }
                    )
            });
            let expected_home = if reference_only {
                None
            } else {
                Some(aggregate_results::result_home(
                    caller,
                    expected_result.place,
                    plan,
                )?)
            };
            let expected_references = reference_custody::reference_results(
                caller,
                callee,
                structural_arguments,
                expected_result.structural_type,
                custody,
                &plan.structural_types,
            )?;
            if result != expected_result
                || callee.result.structural() != Some(callee_result)
                || result.structural_type != callee_result.structural_type
                || result.multiplicity != callee_result.multiplicity
                || *result_home != expected_home
                || *reference_results != expected_references
                // These remain explicit implementation limits. Equality with
                // the source alone would accidentally widen native admission.
                || !claim_transfers.is_empty()
                || !returned_claim_transfers.is_empty()
                || !expected_returns.is_empty()
                || !requirement_obligations.is_empty()
                || !crash_continuations.is_empty()
                || !selected_evidence.is_empty()
            {
                return Err(invalid());
            }
        }
        _ => return Err(invalid()),
    }
    Ok(())
}

fn scalar(
    retained: TargetUnitScalarHomeRequirement,
    operation: semantic_vocabulary::OperationId,
    result: AbstractResult,
    call_plan: &CallPlan,
) -> Result<(), LegalizationError> {
    let invalid = || LegalizationError::SourceCustodyMismatch;
    let shape = scalar_shape(result.scalar_type).ok_or_else(invalid)?;
    let expected = TargetUnitScalarHomeRequirement {
        defining_operation: operation,
        source_value: result.value,
        scalar_type: result.scalar_type,
        shape,
    };
    if retained != expected
        || call_plan.result.as_ref().map(|placement| placement.shape) != Some(shape)
    {
        return Err(invalid());
    }
    Ok(())
}
