use super::LoweringError;
use super::payloadless::exact_unrestricted_payloadless_result;
use super::structural::lower_structural_machine;
use crate::shared::*;

pub(super) fn lower_machine(
    module: &psi_terminal::TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
    retain_payloadless_for_optimization: bool,
) -> Result<AbstractFunction, LoweringError> {
    if !retain_payloadless_for_optimization
        && let Some(operation) = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::EstablishPayloadlessCase { .. }
                ) || matches!(operation.kind, OperationKind::CallStructural { .. })
                    && operation.result.structural().is_some_and(|result| {
                        result.multiplicity == psi_terminal::StructuralMultiplicity::Unrestricted
                    })
            })
    {
        return Err(LoweringError::UnsupportedPayloadlessCase(operation.id));
    }
    if let Some(result) = machine.result.structural()
        && !(retain_payloadless_for_optimization
            && exact_unrestricted_payloadless_result(module, machine, machines))
    {
        return lower_structural_machine(machine, result, structural_types);
    }
    let result = machine.result.scalar();
    let blocks = machine
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let mut operations = Vec::new();
    let mut block_entries = Vec::with_capacity(machine.blocks.len());
    let value_types = machine
        .parameters
        .iter()
        .chain(result.iter())
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| block.parameters.iter()),
        )
        .chain(machine.blocks.iter().flat_map(|block| {
            block
                .operations
                .iter()
                .filter_map(|operation| operation.result.scalar_ref())
        }))
        .map(|value| (value.id, value.scalar_type))
        .collect::<BTreeMap<_, _>>();
    let byte_sequence_literals = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                structural_type,
            } => Some((place, declaration_ordinal, structural_type)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let unit_affine_locals = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            psi_core::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                structural_type,
            } => Some((place, declaration_ordinal, structural_type)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut lowered_unit_affine_locals = Vec::new();
    let mut lowered_byte_sequence_literals = 0_usize;

    for block in &machine.blocks {
        block_entries.push(AbstractBlockEntry {
            block: block.id,
            parameters: block
                .parameters
                .iter()
                .map(|parameter| AbstractParameter {
                    value: parameter.id,
                    scalar_type: parameter.scalar_type,
                })
                .collect(),
            operation_offset: operations.len(),
        });
        for operation in &block.operations {
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
                    if usize::try_from(ordinal) != Ok(lowered_byte_sequence_literals)
                        || !matches!(
                            declaration.shape,
                            psi_terminal::StructuralTypeShape::ByteSequence(
                                psi_terminal::ByteSequenceCarrier::BorrowedView
                            )
                        )
                    {
                        return Err(LoweringError::UnsupportedByteSequenceLiteral(operation.id));
                    }
                    lowered_byte_sequence_literals += 1;
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
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
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
                    let Some(ScalarType::Integer(source_type)) = value_types.get(&operand).copied()
                    else {
                        return Err(LoweringError::VerifiedIntegerWidenMalformed(operation.id));
                    };
                    let ScalarType::Integer(target_type) =
                        operation.result.expect_scalar().scalar_type
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
                    let Some(ScalarType::Integer(source_type)) = value_types.get(&operand).copied()
                    else {
                        return Err(LoweringError::VerifiedIntegerExactCastMalformed(
                            operation.id,
                        ));
                    };
                    let ScalarType::Integer(target_type) =
                        operation.result.expect_scalar().scalar_type
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
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedIntegerBitwiseMalformed(operation.id));
                    };
                    operations.push(match operation.kind.clone() {
                        OperationKind::IntegerBitwiseAnd { .. } => {
                            AbstractOperation::IntegerBitwiseAnd {
                                psi_operation: operation.id,
                                result: operation.result.expect_scalar().id,
                                scalar_type,
                                left,
                                right,
                            }
                        }
                        OperationKind::IntegerBitwiseOr { .. } => {
                            AbstractOperation::IntegerBitwiseOr {
                                psi_operation: operation.id,
                                result: operation.result.expect_scalar().id,
                                scalar_type,
                                left,
                                right,
                            }
                        }
                        OperationKind::IntegerBitwiseXor { .. } => {
                            AbstractOperation::IntegerBitwiseXor {
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
                OperationKind::WrappingIntegerShiftLeft { value, count }
                | OperationKind::WrappingIntegerShiftRight { value, count } => {
                    let ScalarType::Integer(value_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedWrappingShiftMalformed(operation.id));
                    };
                    let Some(ScalarType::Integer(count_type)) = value_types.get(&count).copied()
                    else {
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
                    let ScalarType::Integer(value_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedExactShiftMalformed(operation.id));
                    };
                    let Some(ScalarType::Integer(count_type)) = value_types.get(&count).copied()
                    else {
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
                    let ScalarType::Integer(value_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedExactShiftMalformed(operation.id));
                    };
                    let Some(ScalarType::Integer(count_type)) = value_types.get(&count).copied()
                    else {
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
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
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
                        OperationKind::WrappingIntegerAdd { .. } => {
                            AbstractOperation::WrappingIntegerAdd {
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
                OperationKind::ExactIntegerSubtract { left, right, .. }
                | OperationKind::WrappingIntegerSubtract { left, right } => {
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
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
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
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
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
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
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
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
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
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
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
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
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
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
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
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
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
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
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
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
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
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
        }
        match &block.terminator {
            Terminator::Jump {
                edge,
                target,
                arguments,
                trivial_affine_discards,
            } => {
                let target_block =
                    blocks
                        .get(target)
                        .copied()
                        .ok_or(LoweringError::VerifiedBlockMissing {
                            machine: machine.id,
                            block: *target,
                        })?;
                if target_block.parameters.len() != arguments.len() {
                    return Err(LoweringError::VerifiedJumpArityMismatch { edge: *edge });
                }
                operations.push(AbstractOperation::Jump {
                    psi_edge: *edge,
                    target: *target,
                    bindings: target_block
                        .parameters
                        .iter()
                        .zip(arguments)
                        .map(|(parameter, argument)| ValueBinding {
                            parameter: parameter.id,
                            argument: *argument,
                            scalar_type: parameter.scalar_type,
                        })
                        .collect(),
                    trivial_affine_discards: trivial_affine_discards.clone(),
                });
            }
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                let lower_successor = |successor: &psi_terminal::SuccessorEdge| {
                    let target_block = blocks.get(&successor.target).copied().ok_or(
                        LoweringError::VerifiedBlockMissing {
                            machine: machine.id,
                            block: successor.target,
                        },
                    )?;
                    if target_block.parameters.len() != successor.arguments.len() {
                        return Err(LoweringError::VerifiedJumpArityMismatch {
                            edge: successor.edge,
                        });
                    }
                    Ok(AbstractSuccessor {
                        psi_edge: successor.edge,
                        target: successor.target,
                        bindings: target_block
                            .parameters
                            .iter()
                            .zip(&successor.arguments)
                            .map(|(parameter, argument)| ValueBinding {
                                parameter: parameter.id,
                                argument: *argument,
                                scalar_type: parameter.scalar_type,
                            })
                            .collect(),
                        trivial_affine_discards: successor.trivial_affine_discards.clone(),
                    })
                };
                operations.push(AbstractOperation::Conditional {
                    condition: *condition,
                    when_true: lower_successor(when_true)?,
                    when_false: lower_successor(when_false)?,
                });
            }
            Terminator::Return {
                edge,
                value,
                cleanup_actions,
            } => {
                let result =
                    result.ok_or(LoweringError::ScalarReturnFromUnitMachine(machine.id))?;
                operations.push(AbstractOperation::Return {
                    psi_edge: *edge,
                    result: result.id,
                    value: *value,
                    scalar_type: result.scalar_type,
                    cleanup_actions: cleanup_actions
                        .iter()
                        .cloned()
                        .map(|action| match action {
                            TerminalAffineCleanupAction::InvokeNominal(mut cleanup) => {
                                // Psi has already verified these proof-site identities. They
                                // carry no native realization meaning and must not become a
                                // second semantic authority in Omega artifacts.
                                cleanup.cleanup_receiver = None;
                                cleanup.requirement_obligations.clear();
                                TerminalAffineCleanupAction::InvokeNominal(cleanup)
                            }
                            action => action,
                        })
                        .collect(),
                });
            }
            Terminator::ReturnUnit {
                edge,
                trivial_affine_discards,
            } => {
                if result.is_some() {
                    return Err(LoweringError::UnitReturnFromScalarMachine(machine.id));
                }
                let expected_locals = lowered_unit_affine_locals
                    .iter()
                    .rev()
                    .map(|(_, place, _)| place.id)
                    .collect::<Vec<_>>();
                if !trivial_affine_discards.starts_with(&expected_locals) {
                    return Err(LoweringError::UnsupportedStructuralReturn {
                        machine: machine.id,
                        edge: *edge,
                    });
                }
                operations.push(AbstractOperation::ReturnUnit {
                    psi_edge: *edge,
                    cleanup_actions: trivial_affine_discards
                        .iter()
                        .copied()
                        .map(TerminalAffineCleanupAction::DiscardRoot)
                        .collect(),
                });
            }
            Terminator::ReturnUnitPartialAffine {
                edge,
                trivial_affine_discards,
                residual_affine_discards,
            } => {
                if result.is_some() {
                    return Err(LoweringError::UnitReturnFromScalarMachine(machine.id));
                }
                let expected_locals = lowered_unit_affine_locals
                    .iter()
                    .rev()
                    .map(|(_, place, _)| place.id)
                    .collect::<Vec<_>>();
                if !trivial_affine_discards.starts_with(&expected_locals) {
                    return Err(LoweringError::UnsupportedStructuralReturn {
                        machine: machine.id,
                        edge: *edge,
                    });
                }
                operations.push(AbstractOperation::ReturnUnit {
                    psi_edge: *edge,
                    cleanup_actions: trivial_affine_discards
                        .iter()
                        .copied()
                        .map(TerminalAffineCleanupAction::DiscardRoot)
                        .chain(
                            residual_affine_discards
                                .iter()
                                .cloned()
                                .map(TerminalAffineCleanupAction::DiscardResidual),
                        )
                        .collect(),
                });
            }
            Terminator::ReturnUnitNominalAffine { edge, cleanups } => {
                if result.is_some() || !lowered_unit_affine_locals.is_empty() {
                    return Err(LoweringError::UnsupportedStructuralReturn {
                        machine: machine.id,
                        edge: *edge,
                    });
                }
                operations.push(AbstractOperation::ReturnUnit {
                    psi_edge: *edge,
                    cleanup_actions: cleanups
                        .iter()
                        .cloned()
                        .map(|mut cleanup| {
                            // Psi has already verified these proof-site identities. They
                            // carry no native realization meaning and must not become a
                            // second semantic authority in Omega artifacts.
                            cleanup.cleanup_receiver = None;
                            cleanup.requirement_obligations.clear();
                            TerminalAffineCleanupAction::InvokeNominal(cleanup)
                        })
                        .collect(),
                });
            }
            Terminator::ReturnStructural {
                edge,
                source,
                returned_claims,
                trivial_affine_discards,
            } if retain_payloadless_for_optimization
                && machine.result.structural().is_some_and(|result| {
                    result.multiplicity == StructuralMultiplicity::Unrestricted
                }) =>
            {
                operations.push(AbstractOperation::ReturnStructural {
                    psi_edge: *edge,
                    source: *source,
                    returned_claims: returned_claims.clone(),
                    trivial_affine_locals: Vec::new(),
                    trivial_affine_discards: trivial_affine_discards.clone(),
                });
            }
            Terminator::ReturnStructural { edge, .. } => {
                return Err(LoweringError::UnsupportedStructuralReturn {
                    machine: machine.id,
                    edge: *edge,
                });
            }
            Terminator::Crash {
                edge,
                cause,
                site_guard,
                frontier_lower_bound,
            } => {
                operations.push(AbstractOperation::Crash {
                    psi_edge: *edge,
                    cause: *cause,
                    site_guard: site_guard.clone(),
                    frontier_lower_bound: frontier_lower_bound.clone(),
                });
            }
        }
    }

    Ok(AbstractFunction {
        machine: machine.id,
        attachment: machine.attachment,
        entry: machine.entry,
        parameters: machine
            .parameters
            .iter()
            .map(|parameter| AbstractParameter {
                value: parameter.id,
                scalar_type: parameter.scalar_type,
            })
            .collect(),
        structural_parameters: machine.structural_parameters.clone(),
        result: match &machine.result {
            psi_terminal::TerminalMachineResult::Unit => AbstractFunctionResult::Unit,
            psi_terminal::TerminalMachineResult::Scalar(result) => {
                AbstractFunctionResult::Scalar(AbstractResult {
                    value: result.id,
                    scalar_type: result.scalar_type,
                })
            }
            psi_terminal::TerminalMachineResult::Structural(result) => {
                AbstractFunctionResult::Structural(result.clone())
            }
        },
        entry_claims: machine.entry_claims.clone(),
        published_service_ceiling: machine.published_service_ceiling.clone(),
        block_entries,
        operations,
    })
}
