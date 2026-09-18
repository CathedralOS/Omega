//! Call and hosted boundary instructions replayed against the abstract
//! call each legalizes.

use super::super::{
    AbstractOperation, AbstractOperationPlan, Error, LegalizedOperationPlan,
    LegalizedScalarArgument, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    NativeCallOrigin, PsiOptimizationUnit, TargetOperationPlan,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use semantic_vocabulary::OperationId;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_call(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed_plan: &LegalizedOperationPlan,
    operation: OperationId,
    custody: &scalar_graph_input::reference_custody::Custody,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::Call(call),
        AbstractOperation::CallUnit {
            callee,
            arguments: scalar_arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            ..
        }
        | AbstractOperation::CallStructuralScalar {
            callee,
            arguments: scalar_arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_call")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    let called = unit
        .functions
        .iter()
        .find(|function| function.machine == *callee)
        .ok_or(invalid.clone())?;
    if called.parameters.len() != scalar_arguments.len()
        || called.structural_parameters.len() != structural_arguments.len()
    {
        return Err(invalid);
    }
    let expected = scalar_graph_input::callee_plan(*callee, native, plan, unit)?;
    for (position, (argument, actual)) in structural_arguments
        .iter()
        .zip(call.arguments.iter().skip(scalar_arguments.len()))
        .enumerate()
    {
        let LegalizedScalarArgument::Structural { semantic, target } = actual else {
            return Err(invalid);
        };
        if semantic != argument {
            return Err(invalid);
        }
        if scalar_graph_input::structural_call::argument_at(
            argument, position, operation, optimized, called, &expected, native, plan, custody,
        )? != *target
        {
            return Err(invalid);
        }
    }
    if call.arguments.len() != scalar_arguments.len() + structural_arguments.len()
        || expected.parameters.len() != call.arguments.len()
        || call
            .arguments
            .iter()
            .zip(scalar_arguments)
            .zip(&expected.parameters)
            .any(|((actual, source), placement)| {
                !matches!(actual,
                LegalizedScalarArgument::Scalar { source: value, placement: actual }
                if value == source && actual == placement)
            })
        || call.callee != *callee
        || call.call_plan != expected
        || call.result_placement != expected.result
        || call.source != NativeCallOrigin::Authored
        || call.claim_transfers != *claim_transfers
        || call.requirement_obligations != *requirement_obligations
        || call.crash_continuations != *crash_continuations
        || proposed_plan
            .scalar_functions
            .iter()
            .filter(|function| function.machine == *callee)
            .count()
            != 1
    {
        return Err(invalid);
    }
    Ok(())
}

pub(super) fn validate_call_call(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed_plan: &LegalizedOperationPlan,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::Call(call),
        AbstractOperation::Call {
            callee,
            arguments,
            requirement_obligations,
            crash_continuations,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_call_call")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    let expected = scalar_graph_input::callee_plan(*callee, native, plan, unit)?;
    if call.callee != *callee
        || call.call_plan != expected
        || call.result_placement != expected.result
        || call.source != NativeCallOrigin::Authored
        || !call.claim_transfers.is_empty()
        || call.requirement_obligations != *requirement_obligations
        || call.crash_continuations != *crash_continuations
        || call.arguments.len() != arguments.len()
        || call
            .arguments
            .iter()
            .zip(arguments)
            .zip(&expected.parameters)
            .any(|((actual, source), placement)| {
                !matches!(actual, LegalizedScalarArgument::Scalar {source: value,placement: actual} if value == source && actual == placement)
            })
        || proposed_plan
            .scalar_functions
            .iter()
            .filter(|function| function.machine == *callee)
            .count()
            != 1
    {
        return Err(invalid);
    }
    Ok(())
}
