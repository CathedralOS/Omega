//! Exhaustive Terminal-operation to abstract-operation projection.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_operation(
    operation: &psi_terminal::Operation,
    block: &psi_terminal::Block,
    machine: &TerminalMachine,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
    retain_payloadless_for_optimization: bool,
    value_types: &BTreeMap<psi_core::ValueId, ScalarType>,
    byte_sequence_literals: &[(
        &psi_terminal::StructuralPlaceDeclaration,
        u32,
        psi_core::StructuralTypeId,
    )],
    unit_affine_locals: &[(
        &psi_terminal::StructuralPlaceDeclaration,
        u32,
        psi_core::StructuralTypeId,
    )],
    lowered_unit_affine_locals: &mut Vec<(
        OperationId,
        psi_terminal::StructuralPlaceDeclaration,
        psi_terminal::StructuralTypeDeclaration,
    )>,
    lowered_byte_sequence_literals: &mut usize,
    operations: &mut Vec<AbstractOperation>,
) -> Result<(), LoweringError> {
    match operation.kind.clone() {
        OperationKind::EstablishPayloadlessCase { result_case } => {
            if !retain_payloadless_for_optimization {
                return Err(LoweringError::UnsupportedPayloadlessCase(operation.id));
            }
            let Some(result) = operation.result.structural().cloned() else {
                return Err(LoweringError::UnsupportedPayloadlessCase(operation.id));
            };
            operations.push(AbstractOperation::EstablishPayloadlessCase {
                psi_operation: operation.id,
                result,
                result_case,
            });
        }
        OperationKind::EstablishByteSequenceLiteral { destination, bytes } => {
            let (place, ordinal, structural_type) = byte_sequence_literals
                .iter()
                .find(|(place, _, _)| place.id == destination)
                .copied()
                .ok_or(LoweringError::UnsupportedByteSequenceLiteral(operation.id))?;
            let declaration = structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
                .cloned()
                .ok_or(LoweringError::UnsupportedByteSequenceLiteral(operation.id))?;
            if usize::try_from(ordinal) != Ok(*lowered_byte_sequence_literals)
                || !matches!(
                    declaration.shape,
                    psi_terminal::StructuralTypeShape::ByteSequence(
                        psi_terminal::ByteSequenceCarrier::BorrowedView
                    )
                )
            {
                return Err(LoweringError::UnsupportedByteSequenceLiteral(operation.id));
            }
            *lowered_byte_sequence_literals += 1;
            operations.push(AbstractOperation::EstablishByteSequenceLiteral {
                psi_operation: operation.id,
                place: *place,
                structural_type: declaration,
                bytes,
            });
        }
        OperationKind::EstablishTrivialAffineLocal { destination } => {
            let (place, ordinal, structural_type) = unit_affine_locals
                .iter()
                .find(|(place, _, _)| place.id == destination)
                .copied()
                .ok_or(LoweringError::UnsupportedStructuralReturn {
                    machine: machine.id,
                    edge: block.terminator.edge(),
                })?;
            let declaration = structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
                .cloned()
                .ok_or(LoweringError::UnsupportedStructuralReturn {
                    machine: machine.id,
                    edge: block.terminator.edge(),
                })?;
            if usize::try_from(ordinal) != Ok(lowered_unit_affine_locals.len())
                || !matches!(declaration.shape, psi_terminal::StructuralTypeShape::Record { ref fields } if fields.is_empty())
            {
                return Err(LoweringError::UnsupportedStructuralReturn {
                    machine: machine.id,
                    edge: block.terminator.edge(),
                });
            }
            lowered_unit_affine_locals.push((operation.id, *place, declaration.clone()));
            operations.push(AbstractOperation::EstablishTrivialAffineLocal {
                psi_operation: operation.id,
                place: *place,
                structural_type: declaration,
            });
        }
        OperationKind::CallUnit {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } => {
            operations.push(AbstractOperation::CallUnit {
                psi_operation: operation.id,
                callee,
                structural_arguments,
                claim_transfers,
            });
        }
        OperationKind::CallStructuralScalar {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } => {
            let result = operation.result.expect_scalar();
            operations.push(AbstractOperation::CallStructuralScalar {
                psi_operation: operation.id,
                result: AbstractResult {
                    value: result.id,
                    scalar_type: result.scalar_type,
                },
                callee,
                structural_arguments,
                claim_transfers,
            });
        }
        OperationKind::CallStructural {
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
            selected_evidence,
        } => {
            let Some(result) = operation.result.structural().cloned() else {
                return Err(LoweringError::UnsupportedStructuralResult(machine.id));
            };
            operations.push(AbstractOperation::CallStructural {
                psi_operation: operation.id,
                result,
                callee,
                structural_arguments,
                claim_transfers,
                returned_claim_transfers,
                requirement_obligations,
                crash_continuations,
                selected_evidence,
            });
        }
        OperationKind::BoundaryCall {
            boundary,
            arguments,
            structural_arguments,
            completion_receipts,
            ..
        } => {
            let mut completion_claim_sources = machine
                .entry_claims
                .iter()
                .cloned()
                .map(|entry| CompletionClaimSource {
                    claim: entry.claim,
                    entry: Some(entry),
                    content: None,
                })
                .collect::<Vec<_>>();
            for content in &machine.content_entry_claims {
                if let Some(source) = completion_claim_sources
                    .iter_mut()
                    .find(|source| source.claim == content.claim)
                {
                    source.content = Some(content.clone());
                } else {
                    completion_claim_sources.push(CompletionClaimSource {
                        claim: content.claim,
                        entry: None,
                        content: Some(content.clone()),
                    });
                }
            }
            completion_claim_sources.sort();
            operations.push(AbstractOperation::BoundaryCall {
                psi_operation: operation.id,
                result: operation.result.scalar().map(|result| AbstractResult {
                    value: result.id,
                    scalar_type: result.scalar_type,
                }),
                boundary,
                arguments,
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
            });
        }
        OperationKind::PortWrite {
            service,
            port,
            value,
        } => {
            operations.push(AbstractOperation::PortWrite {
                psi_operation: operation.id,
                service,
                port,
                value,
            });
        }
        OperationKind::WriteOnlyPrimitiveStore { .. } => {
            return Err(LoweringError::UnsupportedWriteOnlyPrimitiveStore(
                operation.id,
            ));
        }
        OperationKind::Call {
            callee, arguments, ..
        } => {
            operations.push(AbstractOperation::Call {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                scalar_type: operation.result.expect_scalar().scalar_type,
                callee,
                arguments,
            });
        }
        OperationKind::IntegerConstant { value } => {
            operations.push(AbstractOperation::IntegerConstant {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                scalar_type: operation.result.expect_scalar().scalar_type,
                value,
            });
        }
        OperationKind::BooleanConstant { value } => {
            operations.push(AbstractOperation::BooleanConstant {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                value,
            });
        }
        OperationKind::BooleanStructuralField { source, field } => {
            operations.push(AbstractOperation::BooleanStructuralField {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                source,
                field,
            });
        }
        OperationKind::BooleanNot { operand } => {
            operations.push(AbstractOperation::BooleanNot {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                operand,
            });
        }
        OperationKind::BooleanEqual { left, right } => {
            operations.push(AbstractOperation::BooleanEqual {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                left,
                right,
            });
        }
        OperationKind::IntegerEqual { left, right } => {
            operations.push(AbstractOperation::IntegerEqual {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                left,
                right,
            });
        }
        OperationKind::IntegerLessThan { left, right } => {
            operations.push(AbstractOperation::IntegerLessThan {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                left,
                right,
            });
        }
        OperationKind::IntegerLessOrEqual { left, right } => {
            operations.push(AbstractOperation::IntegerLessOrEqual {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                left,
                right,
            });
        }
        OperationKind::IntegerBitwiseNot { operand } => {
            let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedIntegerBitwiseMalformed(operation.id));
            };
            operations.push(AbstractOperation::IntegerBitwiseNot {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                scalar_type,
                operand,
            });
        }
        OperationKind::IntegerWiden { operand } => {
            let Some(ScalarType::Integer(source_type)) = value_types.get(&operand).copied() else {
                return Err(LoweringError::VerifiedIntegerWidenMalformed(operation.id));
            };
            let ScalarType::Integer(target_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedIntegerWidenMalformed(operation.id));
            };
            operations.push(AbstractOperation::IntegerWiden {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                source_type,
                target_type,
                operand,
            });
        }
        OperationKind::IntegerExactCast {
            operand,
            obligation,
        } => {
            let Some(ScalarType::Integer(source_type)) = value_types.get(&operand).copied() else {
                return Err(LoweringError::VerifiedIntegerExactCastMalformed(
                    operation.id,
                ));
            };
            let ScalarType::Integer(target_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedIntegerExactCastMalformed(
                    operation.id,
                ));
            };
            operations.push(AbstractOperation::IntegerExactCast {
                psi_operation: operation.id,
                obligation,
                result: operation.result.expect_scalar().id,
                source_type,
                target_type,
                operand,
            });
        }
        OperationKind::IntegerBitwiseAnd { left, right }
        | OperationKind::IntegerBitwiseOr { left, right }
        | OperationKind::IntegerBitwiseXor { left, right } => {
            let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedIntegerBitwiseMalformed(operation.id));
            };
            operations.push(match operation.kind.clone() {
                OperationKind::IntegerBitwiseAnd { .. } => AbstractOperation::IntegerBitwiseAnd {
                    psi_operation: operation.id,
                    result: operation.result.expect_scalar().id,
                    scalar_type,
                    left,
                    right,
                },
                OperationKind::IntegerBitwiseOr { .. } => AbstractOperation::IntegerBitwiseOr {
                    psi_operation: operation.id,
                    result: operation.result.expect_scalar().id,
                    scalar_type,
                    left,
                    right,
                },
                OperationKind::IntegerBitwiseXor { .. } => AbstractOperation::IntegerBitwiseXor {
                    psi_operation: operation.id,
                    result: operation.result.expect_scalar().id,
                    scalar_type,
                    left,
                    right,
                },
                _ => unreachable!(),
            });
        }
        OperationKind::WrappingIntegerShiftLeft { value, count }
        | OperationKind::WrappingIntegerShiftRight { value, count } => {
            let ScalarType::Integer(value_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedWrappingShiftMalformed(operation.id));
            };
            let Some(ScalarType::Integer(count_type)) = value_types.get(&count).copied() else {
                return Err(LoweringError::VerifiedWrappingShiftMalformed(operation.id));
            };
            operations.push(match operation.kind.clone() {
                OperationKind::WrappingIntegerShiftLeft { .. } => {
                    AbstractOperation::WrappingIntegerShiftLeft {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        value_type,
                        count_type,
                        value,
                        count,
                    }
                }
                OperationKind::WrappingIntegerShiftRight { .. } => {
                    AbstractOperation::WrappingIntegerShiftRight {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        value_type,
                        count_type,
                        value,
                        count,
                    }
                }
                _ => unreachable!(),
            });
        }
        OperationKind::ExactIntegerShiftRight {
            value,
            count,
            obligation,
        } => {
            let ScalarType::Integer(value_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedExactShiftMalformed(operation.id));
            };
            let Some(ScalarType::Integer(count_type)) = value_types.get(&count).copied() else {
                return Err(LoweringError::VerifiedExactShiftMalformed(operation.id));
            };
            operations.push(AbstractOperation::ExactIntegerShiftRight {
                psi_operation: operation.id,
                obligation,
                result: operation.result.expect_scalar().id,
                value_type,
                count_type,
                value,
                count,
            });
        }
        OperationKind::ExactIntegerShiftLeft {
            value,
            count,
            obligation,
        } => {
            let ScalarType::Integer(value_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedExactShiftMalformed(operation.id));
            };
            let Some(ScalarType::Integer(count_type)) = value_types.get(&count).copied() else {
                return Err(LoweringError::VerifiedExactShiftMalformed(operation.id));
            };
            operations.push(AbstractOperation::ExactIntegerShiftLeft {
                psi_operation: operation.id,
                obligation,
                result: operation.result.expect_scalar().id,
                value_type,
                count_type,
                value,
                count,
            });
        }
        OperationKind::ExactIntegerAdd { left, right, .. }
        | OperationKind::WrappingIntegerAdd { left, right } => {
            let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedWrappingAddMalformed(operation.id));
            };
            operations.push(match operation.kind.clone() {
                OperationKind::ExactIntegerAdd { obligation, .. } => {
                    AbstractOperation::ExactIntegerAdd {
                        psi_operation: operation.id,
                        obligation,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        left,
                        right,
                    }
                }
                OperationKind::WrappingIntegerAdd { .. } => AbstractOperation::WrappingIntegerAdd {
                    psi_operation: operation.id,
                    result: operation.result.expect_scalar().id,
                    scalar_type,
                    left,
                    right,
                },
                _ => unreachable!(),
            });
        }
        OperationKind::ExactIntegerSubtract { left, right, .. }
        | OperationKind::WrappingIntegerSubtract { left, right } => {
            let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedWrappingSubtractMalformed(
                    operation.id,
                ));
            };
            operations.push(match operation.kind.clone() {
                OperationKind::ExactIntegerSubtract { obligation, .. } => {
                    AbstractOperation::ExactIntegerSubtract {
                        psi_operation: operation.id,
                        obligation,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        left,
                        right,
                    }
                }
                OperationKind::WrappingIntegerSubtract { .. } => {
                    AbstractOperation::WrappingIntegerSubtract {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        left,
                        right,
                    }
                }
                _ => unreachable!(),
            });
        }
        OperationKind::ExactIntegerMultiply { left, right, .. }
        | OperationKind::WrappingIntegerMultiply { left, right } => {
            let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedWrappingMultiplyMalformed(
                    operation.id,
                ));
            };
            operations.push(match operation.kind.clone() {
                OperationKind::ExactIntegerMultiply { obligation, .. } => {
                    AbstractOperation::ExactIntegerMultiply {
                        psi_operation: operation.id,
                        obligation,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        left,
                        right,
                    }
                }
                OperationKind::WrappingIntegerMultiply { .. } => {
                    AbstractOperation::WrappingIntegerMultiply {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        left,
                        right,
                    }
                }
                _ => unreachable!(),
            });
        }
        OperationKind::ExactIntegerDivide {
            left,
            right,
            obligation,
        } => {
            let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedExactDivideMalformed(operation.id));
            };
            operations.push(AbstractOperation::ExactIntegerDivide {
                psi_operation: operation.id,
                obligation,
                result: operation.result.expect_scalar().id,
                scalar_type,
                left,
                right,
            });
        }
        OperationKind::ExactIntegerRemainder {
            left,
            right,
            obligation,
        } => {
            let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedExactRemainderMalformed(operation.id));
            };
            operations.push(AbstractOperation::ExactIntegerRemainder {
                psi_operation: operation.id,
                obligation,
                result: operation.result.expect_scalar().id,
                scalar_type,
                left,
                right,
            });
        }
        OperationKind::WrappingIntegerDivide {
            left,
            right,
            obligation,
        } => {
            let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedWrappingDivideMalformed(operation.id));
            };
            operations.push(AbstractOperation::WrappingIntegerDivide {
                psi_operation: operation.id,
                obligation,
                result: operation.result.expect_scalar().id,
                scalar_type,
                left,
                right,
            });
        }
        OperationKind::WrappingIntegerRemainder {
            left,
            right,
            obligation,
        } => {
            let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedWrappingRemainderMalformed(
                    operation.id,
                ));
            };
            operations.push(AbstractOperation::WrappingIntegerRemainder {
                psi_operation: operation.id,
                obligation,
                result: operation.result.expect_scalar().id,
                scalar_type,
                left,
                right,
            });
        }
        OperationKind::SaturatingIntegerDivide {
            left,
            right,
            obligation,
        } => {
            let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedSaturatingDivideMalformed(
                    operation.id,
                ));
            };
            operations.push(AbstractOperation::SaturatingIntegerDivide {
                psi_operation: operation.id,
                obligation,
                result: operation.result.expect_scalar().id,
                scalar_type,
                left,
                right,
            });
        }
        OperationKind::SaturatingIntegerRemainder {
            left,
            right,
            obligation,
        } => {
            let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedSaturatingRemainderMalformed(
                    operation.id,
                ));
            };
            operations.push(AbstractOperation::SaturatingIntegerRemainder {
                psi_operation: operation.id,
                obligation,
                result: operation.result.expect_scalar().id,
                scalar_type,
                left,
                right,
            });
        }
        OperationKind::SaturatingIntegerAdd { left, right } => {
            let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedSaturatingAddMalformed(operation.id));
            };
            operations.push(AbstractOperation::SaturatingIntegerAdd {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                scalar_type,
                left,
                right,
            });
        }
        OperationKind::SaturatingIntegerSubtract { left, right } => {
            let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedSaturatingSubtractMalformed(
                    operation.id,
                ));
            };
            operations.push(AbstractOperation::SaturatingIntegerSubtract {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                scalar_type,
                left,
                right,
            });
        }
        OperationKind::SaturatingIntegerMultiply { left, right } => {
            let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedSaturatingMultiplyMalformed(
                    operation.id,
                ));
            };
            operations.push(AbstractOperation::SaturatingIntegerMultiply {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                scalar_type,
                left,
                right,
            });
        }
    }
    Ok(())
}
