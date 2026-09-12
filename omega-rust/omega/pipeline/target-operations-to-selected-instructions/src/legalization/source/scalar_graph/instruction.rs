use super::*;
use semantic_vocabulary::{IntegerValue, ScalarType};
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
        AbstractOperation::IeeeFloatCompare {
            comparison,
            format,
            left,
            right,
            ..
        } => LegalizedScalarInstructionKind::IeeeFloatCompare {
            comparison: *comparison,
            format: *format,
            left: *left,
            right: *right,
        },
        AbstractOperation::CallStructural {
            result,
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            ..
        } => {
            let call_plan = scalar_graph_input::callee_plan(*callee, native, plan, unit)?;
            let called = unit
                .functions
                .iter()
                .find(|function| function.machine == *callee)
                .ok_or(Error::SourceCustodyMismatch)?;
            let mut lowered = arguments
                .iter()
                .zip(&call_plan.parameters)
                .map(|(source, placement)| LegalizedScalarArgument::Scalar {
                    source: *source,
                    placement: placement.clone(),
                })
                .collect::<Vec<_>>();
            for (position, semantic) in structural_arguments.iter().enumerate() {
                lowered.push(LegalizedScalarArgument::Structural {
                    semantic: semantic.clone(),
                    target: scalar_graph_input::aggregate_results::call_argument(
                        semantic, position, operation, optimized, called, &call_plan, native, plan,
                    )?,
                });
            }
            LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
                callee: *callee,
                arguments: lowered,
                structural_result: Some(result.clone()),
                result_placement: call_plan.result.clone(),
                call_plan,
                source: LegalizedCallUnitSource::AuthoredCallUnit,
                claim_transfers: claim_transfers.clone(),
                requirement_obligations: requirement_obligations.clone(),
                crash_continuations: crash_continuations.clone(),
            })
        }
        AbstractOperation::EstablishScalarArray {
            result, elements, ..
        } => LegalizedScalarInstructionKind::EstablishScalarArray {
            result: result.clone(),
            elements: elements.clone(),
            shape: scalar_graph_input::scalar_arrays::shape(result, plan)?.2,
        },
        AbstractOperation::EstablishScalarCase {
            result,
            result_case,
            fields,
            ..
        } => LegalizedScalarInstructionKind::EstablishScalarCase {
            result: result.clone(),
            result_case: *result_case,
            fields: fields.clone(),
            layout: scalar_graph_input::aggregate_results::sum_layout(result, plan)?,
        },
        AbstractOperation::EstablishPrimitiveLocal { result, value, .. } => {
            LegalizedScalarInstructionKind::EstablishPrimitiveLocal {
                result: result.clone(),
                value: *value,
                shape: scalar_graph_input::scalar_shape(value.scalar_type)
                    .ok_or(Error::SourceCustodyMismatch)?,
            }
        }
        AbstractOperation::PrimitiveLocalStore {
            destination, value, ..
        } => LegalizedScalarInstructionKind::PrimitiveLocalStore {
            destination: *destination,
            value: *value,
        },
        AbstractOperation::StructuralCaseMembership {
            source,
            case,
            result,
            ..
        } => {
            if result.scalar_type != ScalarType::Boolean {
                return Err(Error::SourceCustodyMismatch);
            }
            LegalizedScalarInstructionKind::StructuralCaseMembership {
                source: *source,
                case: *case,
                case_tag: scalar_graph_input::structural_case::membership_tag(
                    optimized, *source, *case, plan,
                )?,
            }
        }
        AbstractOperation::PrimitiveScalarRead { source, .. } => {
            LegalizedScalarInstructionKind::PrimitiveScalarRead { source: *source }
        }
        AbstractOperation::BoundaryCall {
            boundary,
            result: abstract_operations::AbstractBoundaryResult::Structural(result),
            ..
        } if matches!(
            scalar_graph_input::hosted_realization(native, optimized.machine, operation)?,
            target_operations::BoundaryRealization::HostedReadByte(_)
        ) =>
        {
            LegalizedScalarInstructionKind::HostedReadByte {
                boundary: *boundary,
                result: result.clone(),
                layout: scalar_graph_input::read_byte::layout(result, plan)?,
            }
        }
        AbstractOperation::BoundaryCall {
            boundary,
            arguments,
            ..
        } => {
            let [source] = arguments.as_slice() else {
                return Err(Error::SourceCustodyMismatch);
            };
            match scalar_graph_input::hosted_realization(native, optimized.machine, operation)? {
                target_operations::BoundaryRealization::HostedWriteByteI32(_) => {
                    LegalizedScalarInstructionKind::HostedWriteByteI32 {
                        boundary: *boundary,
                        source: *source,
                    }
                }
                target_operations::BoundaryRealization::HostedExitProcessI32(_) => {
                    LegalizedScalarInstructionKind::HostedExitProcessI32 {
                        boundary: *boundary,
                        source: *source,
                    }
                }
                _ => return Err(Error::SourceCustodyMismatch),
            }
        }
        AbstractOperation::WriteOnlyPrimitiveStore {
            destination, value, ..
        } => {
            let byte_size = crate::structural_reference_input::primitive_store(
                destination,
                value.scalar_type,
                &unit.structural_types,
            )
            .ok_or(Error::SourceCustodyMismatch)?;
            LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
                destination: destination.clone(),
                value: *value,
                byte_size,
            }
        }
        AbstractOperation::StructuralScalarFieldStore {
            destination,
            path,
            field,
            value,
            ..
        } => {
            let (byte_offset, byte_size) = crate::structural_reference_input::store(
                destination.structural_type,
                path,
                *field,
                value.scalar_type,
                &unit.structural_types,
            )
            .ok_or(Error::SourceCustodyMismatch)?;
            LegalizedScalarInstructionKind::StructuralScalarFieldStore {
                destination: destination.clone(),
                path: path.clone(),
                field: *field,
                value: *value,
                byte_offset,
                byte_size,
            }
        }
        AbstractOperation::EstablishByteSequenceLiteral {
            place,
            structural_type,
            bytes,
            ..
        } => LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
            destination: *place,
            structural_type: structural_type.clone(),
            bytes: bytes.clone(),
        },
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
            let call_plan = scalar_graph_input::callee_plan(*callee, native, plan, unit)?;
            let called = unit
                .functions
                .iter()
                .find(|function| function.machine == *callee)
                .ok_or(Error::SourceCustodyMismatch)?;
            if called.parameters.len() != scalar_arguments.len()
                || called.structural_parameters.len() != structural_arguments.len()
                || call_plan.parameters.len() != scalar_arguments.len() + structural_arguments.len()
            {
                return Err(Error::SourceCustodyMismatch);
            }
            let mut arguments = scalar_arguments
                .iter()
                .zip(&call_plan.parameters)
                .map(|(source, placement)| LegalizedScalarArgument::Scalar {
                    source: *source,
                    placement: placement.clone(),
                })
                .collect::<Vec<_>>();
            for (position, semantic) in structural_arguments.iter().enumerate() {
                let target = scalar_graph_input::structural_call::argument_at(
                    semantic, position, operation, optimized, called, &call_plan, native, plan,
                )?;
                arguments.push(LegalizedScalarArgument::Structural {
                    semantic: semantic.clone(),
                    target,
                });
            }
            LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
                structural_result: None,
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
        AbstractOperation::ByteSequenceWrite {
            psi_operation,
            destination,
            index,
            value,
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
            LegalizedScalarInstructionKind::ByteSequenceWrite {
                destination: *destination,
                index: *index,
                value: *value,
                length: *length,
                obligation: *obligation,
                accepted_fact: fact.identity,
            }
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
        AbstractOperation::IntegerExactCast {
            psi_operation,
            operand,
            source_type,
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
            if !optimized.facts.iter().any(|fact| matches!(fact,
                optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
                if referenced == obligation && support == psi_operation)) {
                return Err(Error::SourceCustodyMismatch);
            }
            LegalizedScalarInstructionKind::IntegerExactCast {
                operand: *operand,
                source_type: *source_type,
                obligation: *obligation,
                accepted_fact: fact.identity,
            }
        }
        AbstractOperation::IntegerConstant { value, .. } => {
            LegalizedScalarInstructionKind::Constant(*value)
        }
        AbstractOperation::IeeeFloatConstant { value, .. } => {
            let bits = match value {
                semantic_vocabulary::IeeeFloatValue::Binary32(bits) => u128::from(*bits),
                semantic_vocabulary::IeeeFloatValue::Binary64(bits) => u128::from(*bits),
            };
            LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(bits))
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
                structural_result: None,
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
        AbstractOperation::BooleanEqual { left, right, .. }
        | AbstractOperation::IntegerEqual { left, right, .. }
        | AbstractOperation::IntegerLessThan { left, right, .. }
        | AbstractOperation::IntegerLessOrEqual { left, right, .. } => {
            let predicate = match node.operation {
                AbstractOperation::BooleanEqual { .. } | AbstractOperation::IntegerEqual { .. } => {
                    LegalizedScalarComparison::Equal
                }
                AbstractOperation::IntegerLessThan { .. } => LegalizedScalarComparison::LessThan,
                AbstractOperation::IntegerLessOrEqual { .. } => {
                    LegalizedScalarComparison::LessOrEqual
                }
                _ => return Err(Error::SourceCustodyMismatch),
            };
            let operand_type = scalar_graph_input::value_type(optimized, *left)
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
