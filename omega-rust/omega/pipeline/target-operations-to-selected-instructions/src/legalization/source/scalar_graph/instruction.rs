use super::*;
use semantic_vocabulary::IntegerValue;
pub(super) fn project(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstruction, LegalizationError> {
    let (operation, result) =
        scalar_graph_input::instruction(node).ok_or(Error::SourceCustodyMismatch)?;
    let kind = match &node.operation {
        AbstractOperation::ByteSequenceSubslice {
            psi_operation,
            result,
            source,
            start,
            end,
            length,
            obligation,
        } => {
            let fact = unit
                .accepted_obligation_facts
                .iter()
                .find(|fact| {
                    fact.machine == optimized.machine
                        && fact.operation == *psi_operation
                        && fact.obligation == *obligation
                })
                .ok_or(Error::SourceCustodyMismatch)?;
            LegalizedScalarInstructionKind::ByteSequenceSubslice {
                result: result.clone(),
                source: *source,
                start: *start,
                end: *end,
                length: *length,
                obligation: *obligation,
                accepted_fact: fact.identity,
            }
        }
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
        } => {
            let [semantic] = structural_arguments.as_slice() else {
                return Err(Error::SourceCustodyMismatch);
            };
            let target = scalar_graph_input::structural_call::argument(
                semantic, optimized, *callee, native, plan, unit,
            )?;
            let call_plan = scalar_graph_input::structural_call::validate_argument(
                semantic, &target, optimized, *callee, native, plan, unit,
            )?;
            if call_plan.parameters.len() != scalar_arguments.len() + 1 {
                return Err(Error::SourceCustodyMismatch);
            }
            let arguments = scalar_arguments
                .iter()
                .zip(&call_plan.parameters)
                .map(|(source, placement)| LegalizedScalarArgument::Scalar {
                    source: *source,
                    placement: placement.clone(),
                })
                .chain(std::iter::once(LegalizedScalarArgument::Structural {
                    semantic: semantic.clone(),
                    target,
                }))
                .collect();
            LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
                callee: *callee,
                arguments,
                result_placement: call_plan.result.clone(),
                source: LegalizedCallUnitSource::AuthoredCallUnit,
                claim_transfers: claim_transfers.clone(),
                call_plan,
                requirement_obligations: requirement_obligations.clone(),
                crash_continuations: crash_continuations.clone(),
            })
        }
        AbstractOperation::ByteSequenceRead {
            psi_operation,
            source,
            index,
            length,
            obligation,
            ..
        } => {
            let fact = unit
                .accepted_obligation_facts
                .iter()
                .find(|fact| {
                    fact.machine == optimized.machine
                        && fact.operation == *psi_operation
                        && fact.obligation == *obligation
                })
                .ok_or(Error::SourceCustodyMismatch)?;
            LegalizedScalarInstructionKind::ByteSequenceRead {
                source: *source,
                index: *index,
                length: *length,
                obligation: *obligation,
                accepted_fact: fact.identity,
            }
        }
        AbstractOperation::ByteSequenceLength { source, .. } => {
            LegalizedScalarInstructionKind::ByteSequenceLength {
                source: *source,
                length_byte_offset: 8,
            }
        }
        AbstractOperation::BooleanNot { operand, .. } => {
            LegalizedScalarInstructionKind::BooleanNot { operand: *operand }
        }
        AbstractOperation::IntegerWiden {
            operand,
            source_type,
            ..
        } => LegalizedScalarInstructionKind::IntegerWiden {
            operand: *operand,
            source_type: *source_type,
        },
        AbstractOperation::IntegerConstant { value, .. } => {
            LegalizedScalarInstructionKind::Constant(*value)
        }
        AbstractOperation::BooleanConstant { value, .. } => {
            LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(u128::from(*value)))
        }
        AbstractOperation::Call {
            callee,
            arguments,
            requirement_obligations,
            crash_continuations,
            ..
        } => {
            let call_plan = scalar_graph_input::callee_plan(*callee, native, plan, unit)?;
            LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
                callee: *callee,
                arguments: arguments
                    .iter()
                    .zip(&call_plan.parameters)
                    .map(|(source, placement)| LegalizedScalarArgument::Scalar {
                        source: *source,
                        placement: placement.clone(),
                    })
                    .collect(),
                result_placement: call_plan.result.clone(),
                source: LegalizedCallUnitSource::AuthoredCallUnit,
                claim_transfers: Vec::new(),
                call_plan,
                requirement_obligations: requirement_obligations.clone(),
                crash_continuations: crash_continuations.clone(),
            })
        }
        AbstractOperation::ExactIntegerAdd {
            psi_operation,
            obligation,
            left,
            right,
            ..
        }
        | AbstractOperation::ExactIntegerSubtract {
            psi_operation,
            obligation,
            left,
            right,
            ..
        } => {
            let fact = unit
                .accepted_obligation_facts
                .iter()
                .find(|fact| {
                    fact.machine == optimized.machine
                        && fact.operation == *psi_operation
                        && fact.obligation == *obligation
                })
                .ok_or(Error::SourceCustodyMismatch)?;
            if !optimized.facts.iter().any(|fact| matches!(fact,
                        optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
                        if referenced == obligation && support == psi_operation)) {
                        return Err(Error::SourceCustodyMismatch);
                    }
            LegalizedScalarInstructionKind::ExactBinary {
                operator: if matches!(node.operation, AbstractOperation::ExactIntegerAdd { .. }) {
                    LegalizedExactIntegerOperator::Add
                } else {
                    LegalizedExactIntegerOperator::Subtract
                },
                left: *left,
                right: *right,
                obligation: *obligation,
                accepted_fact: fact.identity,
            }
        }
        AbstractOperation::IntegerEqual { left, right, .. }
        | AbstractOperation::IntegerLessThan { left, right, .. }
        | AbstractOperation::IntegerLessOrEqual { left, right, .. } => {
            let predicate = match node.operation {
                AbstractOperation::IntegerEqual { .. } => LegalizedScalarComparison::Equal,
                AbstractOperation::IntegerLessThan { .. } => LegalizedScalarComparison::LessThan,
                AbstractOperation::IntegerLessOrEqual { .. } => {
                    LegalizedScalarComparison::LessOrEqual
                }
                _ => return Err(Error::SourceCustodyMismatch),
            };
            let operand_type = scalar_graph_input::value_type(optimized, *left)
                .and_then(scalar_graph_input::integer_type)
                .ok_or(Error::SourceCustodyMismatch)?;
            LegalizedScalarInstructionKind::Compare {
                predicate,
                operand_type,
                left: *left,
                right: *right,
            }
        }
        _ => return Err(Error::SourceCustodyMismatch),
    };
    Ok(LegalizedScalarInstruction {
        operation,
        result: result.map(|value| LegalizedValueDefinition {
            value,
            scalar_type: node.definitions[0].scalar_type,
            definition_site: node.definitions[0].site,
        }),
        kind,
        fuel: node.fuel.clone(),
        effect: node.effect,
        ownership: node.ownership.clone(),
    })
}
