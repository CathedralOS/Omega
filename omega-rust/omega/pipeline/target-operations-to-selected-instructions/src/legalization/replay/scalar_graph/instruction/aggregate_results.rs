//! Replay aggregate construction, structural call results and case observations.
//! Observations rejoin the nominal root and case without consuming the aggregate.
use super::{
    AbstractOperation, AbstractOperationPlan, Error, LegalizedScalarArgument,
    LegalizedScalarInstruction, LegalizedScalarInstructionKind, NativeCallOrigin,
    PsiOptimizationUnit, ScalarType, TargetOperationPlan,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;

pub(super) fn validate(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    custody: &scalar_graph_input::reference_custody::Custody,
) -> Result<(), LegalizationError> {
    let invalid = Error::NonCanonicalLegalizedPlan;
    match (&actual.kind, &node.operation) {
        (
            LegalizedScalarInstructionKind::StructuralCaseMembership {
                source,
                path,
                case,
                case_tag,
                tag_byte_offset,
            },
            AbstractOperation::StructuralCaseMembership {
                source: expected,
                path: expected_path,
                case: expected_case,
                result,
                ..
            },
        ) if source == expected
            && path == expected_path
            && case == expected_case
            && result.scalar_type == ScalarType::Boolean
            && (*case_tag, *tag_byte_offset)
                == scalar_graph_input::structural_case::membership_layout(
                    optimized,
                    *expected,
                    expected_path,
                    *expected_case,
                    plan,
                )? => {}
        (
            LegalizedScalarInstructionKind::StructuralLeafCopy {
                result,
                source,
                path,
                byte_offset,
                shape,
                index,
                index_stride,
            },
            AbstractOperation::StructuralLeafCopy {
                source: expected,
                path: expected_path,
                result: expected_result,
                ..
            },
        ) if source == expected && path == expected_path && result == expected_result && {
            let (expected_offset, expected_shape, expected_index) =
                scalar_graph_input::structural_case::leaf_copy_layout(
                    optimized,
                    *expected,
                    expected_path,
                    expected_result,
                    plan,
                )?;
            let expected_index = expected_index
                .map(|(selector, stride)| {
                    let parameter = usize::try_from(selector)
                        .ok()
                        .and_then(|position| optimized.parameters.get(position))
                        .ok_or(LegalizationError::SourceCustodyMismatch)?;
                    Ok::<_, LegalizationError>((
                        Some(abstract_operations::AbstractResult {
                            value: parameter.value,
                            scalar_type: parameter.scalar_type,
                        }),
                        stride,
                    ))
                })
                .transpose()?
                .unwrap_or((None, 0));
            (*byte_offset, *shape) == (expected_offset, expected_shape)
                && (*index, *index_stride) == expected_index
        } => {}
        (
            LegalizedScalarInstructionKind::EstablishScalarArray {
                result,
                elements,
                shape,
            },
            AbstractOperation::EstablishScalarArray {
                result: expected,
                elements: expected_elements,
                ..
            },
        ) if result == expected
            && elements == expected_elements
            && *shape == scalar_graph_input::scalar_arrays::shape(result, plan)?.2 => {}
        (
            LegalizedScalarInstructionKind::Call(call),
            AbstractOperation::CallStructural {
                psi_operation,
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
                || call.source != NativeCallOrigin::Authored
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
                let target = scalar_graph_input::aggregate_results::call_argument(
                    semantic,
                    position,
                    *psi_operation,
                    optimized,
                    called,
                    &expected,
                    native,
                    plan,
                    custody,
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
            LegalizedScalarInstructionKind::EstablishRecord {
                result,
                fields,
                shape,
            },
            AbstractOperation::EstablishRecord {
                result: expected,
                fields: expected_fields,
                ..
            },
        ) if result == expected
            && fields == expected_fields
            && *shape
                == scalar_graph_input::aggregate_results::home_layout(result, plan)?.shape() => {}
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
            && *layout == scalar_graph_input::aggregate_results::sum_layout(result, plan)? => {}
        _ => return Err(invalid),
    }
    Ok(())
}
