use super::*;
use semantic_vocabulary::IntegerValue;
pub(super) fn validate(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed_plan: &LegalizedOperationPlan,
) -> Result<(), LegalizationError> {
    let invalid = Error::NonCanonicalLegalizedPlan;
    let (operation, result) = scalar_graph_input::instruction(node).ok_or(invalid.clone())?;
    if actual.operation != operation
        || actual.result
            != result.map(|value| LegalizedValueDefinition {
                value,
                scalar_type: node.definitions[0].scalar_type,
                definition_site: node.definitions[0].site,
            })
        || actual.fuel != node.fuel
        || actual.effect != node.effect
        || actual.ownership != node.ownership
    {
        return Err(invalid);
    }
    match (&actual.kind, &node.operation) {
        (
            LegalizedScalarInstructionKind::LinuxWriteByteI32 { boundary, source },
            AbstractOperation::BoundaryCall {
                boundary: expected,
                arguments,
                ..
            },
        ) if boundary == expected && arguments.as_slice() == [*source] => {}
        (
            LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
                destination,
                structural_type,
                bytes,
            },
            AbstractOperation::EstablishByteSequenceLiteral {
                place,
                structural_type: expected_type,
                bytes: expected_bytes,
                ..
            },
        ) => {
            if destination != place || structural_type != expected_type || bytes != expected_bytes {
                return Err(invalid);
            }
        }
        (
            LegalizedScalarInstructionKind::ByteSequenceSubslice {
                result,
                source,
                start,
                end,
                length,
                obligation,
                accepted_fact,
            },
            AbstractOperation::ByteSequenceSubslice {
                result: expected_result,
                source: expected_source,
                start: expected_start,
                end: expected_end,
                length: expected_length,
                obligation: expected_obligation,
                ..
            },
        ) => {
            if result != expected_result
                || source != expected_source
                || start != expected_start
                || end != expected_end
                || length != expected_length
                || obligation != expected_obligation
                || !unit.accepted_obligation_facts.iter().any(|fact| {
                    fact.machine == optimized.machine
                        && fact.operation == operation
                        && fact.obligation == *obligation
                        && fact.identity == *accepted_fact
                })
            {
                return Err(invalid);
            }
        }
        (
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
        ) => {
            let ([argument], Some(LegalizedScalarArgument::Structural { semantic, target })) =
                (structural_arguments.as_slice(), call.arguments.last())
            else {
                return Err(invalid);
            };
            let expected = scalar_graph_input::structural_call::validate_argument(
                argument, target, operation, optimized, *callee, native, plan, unit,
            )?;
            if semantic != argument
                || call.arguments.len() != scalar_arguments.len() + 1
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
                || call.source != LegalizedCallUnitSource::AuthoredCallUnit
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
        }
        (
            LegalizedScalarInstructionKind::ByteSequenceRead {
                source,
                index,
                length,
                obligation,
                accepted_fact,
            },
            AbstractOperation::ByteSequenceRead {
                source: expected_source,
                index: expected_index,
                length: expected_length,
                obligation: expected_obligation,
                ..
            },
        ) => {
            if source != expected_source
                || index != expected_index
                || length != expected_length
                || obligation != expected_obligation
                || !unit.accepted_obligation_facts.iter().any(|fact| {
                    fact.machine == optimized.machine
                        && fact.operation == operation
                        && fact.obligation == *obligation
                        && fact.identity == *accepted_fact
                })
            {
                return Err(invalid);
            }
        }
        (
            LegalizedScalarInstructionKind::ByteSequenceLength {
                source,
                length_byte_offset: 8,
            },
            AbstractOperation::ByteSequenceLength {
                source: expected, ..
            },
        ) if source == expected => {}
        (
            LegalizedScalarInstructionKind::BooleanNot { operand },
            AbstractOperation::BooleanNot {
                operand: source, ..
            },
        ) if operand == source => {}
        (
            LegalizedScalarInstructionKind::IntegerWiden {
                operand,
                source_type,
            },
            AbstractOperation::IntegerWiden {
                operand: source,
                source_type: source_integer,
                ..
            },
        ) if operand == source && source_type == source_integer => {}
        (
            LegalizedScalarInstructionKind::Constant(actual),
            AbstractOperation::IntegerConstant { value, .. },
        ) if actual == value => {}
        (
            LegalizedScalarInstructionKind::Constant(actual),
            AbstractOperation::BooleanConstant { value, .. },
        ) if *actual == IntegerValue::Unsigned(u128::from(*value)) => {}
        (
            LegalizedScalarInstructionKind::Call(call),
            AbstractOperation::Call {
                callee,
                arguments,
                requirement_obligations,
                crash_continuations,
                ..
            },
        ) => {
            let expected = scalar_graph_input::callee_plan(*callee, native, plan, unit)?;
            if call.callee != *callee
                || call.call_plan != expected
                || call.result_placement != expected.result
                || call.source != LegalizedCallUnitSource::AuthoredCallUnit
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
        }
        (
            LegalizedScalarInstructionKind::ExactBinary {
                operator,
                left,
                right,
                obligation,
                accepted_fact,
            },
            AbstractOperation::ExactIntegerAdd {
                psi_operation,
                obligation: source_obligation,
                left: source_left,
                right: source_right,
                ..
            }
            | AbstractOperation::ExactIntegerSubtract {
                psi_operation,
                obligation: source_obligation,
                left: source_left,
                right: source_right,
                ..
            },
        ) => {
            let expected_operator = match node.operation {
                AbstractOperation::ExactIntegerAdd { .. } => LegalizedExactIntegerOperator::Add,
                AbstractOperation::ExactIntegerSubtract { .. } => {
                    LegalizedExactIntegerOperator::Subtract
                }
                _ => return Err(invalid),
            };
            let fact = unit
                .accepted_obligation_facts
                .iter()
                .find(|fact| {
                    fact.machine == optimized.machine
                        && fact.operation == *psi_operation
                        && fact.obligation == *source_obligation
                })
                .ok_or(Error::SourceCustodyMismatch)?;
            if *operator != expected_operator || left != source_left || right != source_right
                    || obligation != source_obligation || *accepted_fact != fact.identity
                    || !optimized.facts.iter().any(|fact| matches!(fact,
                        optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
                        if referenced == source_obligation && support == psi_operation)) {
                    return Err(invalid);
                }
        }
        (
            LegalizedScalarInstructionKind::Compare {
                predicate,
                operand_type,
                left,
                right,
            },
            AbstractOperation::IntegerEqual {
                left: source_left,
                right: source_right,
                ..
            }
            | AbstractOperation::IntegerLessThan {
                left: source_left,
                right: source_right,
                ..
            }
            | AbstractOperation::IntegerLessOrEqual {
                left: source_left,
                right: source_right,
                ..
            },
        ) => {
            let expected = match node.operation {
                AbstractOperation::IntegerEqual { .. } => LegalizedScalarComparison::Equal,
                AbstractOperation::IntegerLessThan { .. } => LegalizedScalarComparison::LessThan,
                AbstractOperation::IntegerLessOrEqual { .. } => {
                    LegalizedScalarComparison::LessOrEqual
                }
                _ => return Err(invalid),
            };
            if *predicate != expected
                || left != source_left
                || right != source_right
                || scalar_graph_input::value_type(optimized, *source_left)
                    != Some(ScalarType::Integer(*operand_type))
            {
                return Err(invalid);
            }
        }
        _ => return Err(invalid),
    }
    Ok(())
}
