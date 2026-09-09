//! Replay scalar-sum construction and structural call results independently.
use super::*;

pub(super) fn validate(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let invalid = Error::NonCanonicalLegalizedPlan;
    match (&actual.kind, &node.operation) {
        (
            LegalizedScalarInstructionKind::Call(call),
            AbstractOperation::CallStructural {
                result,
                callee,
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                ..
            },
        ) => {
            let expected = scalar_graph_input::callee_plan(*callee, native, plan, unit)?;
            let called = unit
                .functions
                .iter()
                .find(|function| function.machine == *callee)
                .ok_or(invalid.clone())?;
            if call.callee != *callee
                || call.structural_result.as_ref() != Some(result)
                || call.call_plan != expected
                || call.result_placement != expected.result
                || call.source != LegalizedCallUnitSource::AuthoredCallUnit
                || call.claim_transfers != *claim_transfers
                || call.requirement_obligations != *requirement_obligations
                || call.crash_continuations != *crash_continuations
                || call.arguments.len() != arguments.len() + structural_arguments.len()
            {
                return Err(invalid);
            }
            for (position, value) in arguments.iter().enumerate() {
                if call.arguments[position]
                    != (LegalizedScalarArgument::Scalar {
                        source: *value,
                        placement: expected.parameters[position].clone(),
                    })
                {
                    return Err(invalid);
                }
            }
            for (position, semantic) in structural_arguments.iter().enumerate() {
                let target = scalar_graph_input::scalar_sums::call_argument(
                    semantic, position, optimized, called, &expected, native, plan,
                )?;
                if call.arguments[arguments.len() + position]
                    != (LegalizedScalarArgument::Structural {
                        semantic: semantic.clone(),
                        target,
                    })
                {
                    return Err(invalid);
                }
            }
        }
        (
            LegalizedScalarInstructionKind::EstablishScalarCase {
                result,
                result_case,
                fields,
                layout,
            },
            AbstractOperation::EstablishScalarCase {
                result: expected,
                result_case: expected_case,
                fields: expected_fields,
                ..
            },
        ) if result == expected
            && result_case == expected_case
            && fields == expected_fields
            && *layout == scalar_graph_input::scalar_sums::layout(result, plan)? => {}
        _ => return Err(invalid),
    }
    Ok(())
}
