//! Select explicit control edges without replacing parallel source bindings with copies.
use super::*;
use legalized_operations::{
    LegalizedScalarBlock, LegalizedScalarComparison as Comparison, LegalizedScalarReturnValue,
    LegalizedScalarSuccessor, LegalizedScalarTerminator,
};
use selected_instructions::{SelectedValueBinding, SelectedValueTransport};

pub(super) fn build(
    function: usize,
    source: &LegalizedScalarFunction,
    block: &LegalizedScalarBlock,
    order: &[usize],
    builder: &mut Builder<'_>,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<SelectedTerminator, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::UnsupportedSourceShape { function };
    let constraints = builder.constraints;
    let keys = &constraints.keys;
    match &block.terminator {
        LegalizedScalarTerminator::Return(returned) => {
            let (kind, key, operands, values) = match returned.value {
                LegalizedScalarReturnValue::Unit => (
                    SelectedInstructionKind::ReturnUnit,
                    keys.return_unit,
                    Vec::new(),
                    Vec::new(),
                ),
                LegalizedScalarReturnValue::Value { value, scalar_type } => {
                    let (_, input, site, value_type) =
                        builder.resolve(value).ok_or_else(invalid)?;
                    if value_type != ScalarType::Integer(scalar_type) {
                        return Err(invalid());
                    }
                    let result = source.call_plan.result.as_ref().ok_or_else(invalid)?;
                    let [
                        ValueLocation::Register {
                            register,
                            value_byte_offset: 0,
                            byte_size: 8,
                        },
                    ] = result.locations.as_slice()
                    else {
                        return Err(invalid());
                    };
                    let [operand] = row(builder.catalog, keys.return_i64)?.operands.as_slice()
                    else {
                        return Err(invalid());
                    };
                    if operand.fixed_view.is_none()
                        || operand.fixed_view != environment.fixed_register_view(*register)
                    {
                        return Err(invalid());
                    }
                    let key = keys.return_i64;
                    let output = builder.copy(input, value, site, value_type)?;
                    (
                        SelectedInstructionKind::ReturnI64,
                        key,
                        vec![output],
                        vec![value],
                    )
                }
            };
            let instruction = terminal(
                builder,
                kind,
                key,
                &operands,
                SelectedInstructionProvenance {
                    values,
                    edges: vec![returned.edge],
                    fuel: returned.fuel.clone(),
                    ..Default::default()
                },
            )?;
            Ok(SelectedTerminator::Return {
                instruction,
                psi_return_edge: returned.edge,
            })
        }
        LegalizedScalarTerminator::Jump {
            successor: next, ..
        } => {
            let successor = successor(source, order, builder, next)?;
            let instruction = terminal(
                builder,
                SelectedInstructionKind::Jump,
                keys.jump,
                &[],
                Default::default(),
            )?;
            Ok(SelectedTerminator::Jump {
                instruction,
                successor,
            })
        }
        LegalizedScalarTerminator::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            let mut base = *condition;
            let mut suffix_start = block.instructions.len();
            let mut inverted = false;
            let mut not_rows = Vec::new();
            while let Some(previous) = suffix_start
                .checked_sub(1)
                .and_then(|index| block.instructions.get(index))
            {
                let LegalizedScalarInstructionKind::BooleanNot { operand } = previous.kind else {
                    break;
                };
                if previous.result.as_ref().is_none_or(|result| {
                    result.value != base || result.scalar_type != ScalarType::Boolean
                }) {
                    return Err(invalid());
                }
                base = operand;
                inverted = !inverted;
                not_rows.push(previous);
                suffix_start -= 1;
            }
            not_rows.reverse();
            let comparison = suffix_start
                .checked_sub(1)
                .and_then(|index| block.instructions.get(index))
                .filter(|row| {
                    row.result
                        .as_ref()
                        .is_some_and(|result| result.value == base)
                });
            let (predicate, operand_type) = if let Some(row) = comparison
                && let LegalizedScalarInstructionKind::Compare {
                    predicate,
                    operand_type,
                    ..
                } = row.kind
            {
                (predicate, operand_type)
            } else {
                let (_, input, _, scalar_type) = builder.resolve(base).ok_or_else(invalid)?;
                if scalar_type != ScalarType::Boolean {
                    return Err(invalid());
                }
                builder.emit(
                    SelectedInstructionKind::CompareI64Zero,
                    keys.compare_i64_zero,
                    &[input],
                    SelectedInstructionProvenance {
                        values: vec![base],
                        ..Default::default()
                    },
                )?;
                // A Boolean register is true when nonzero, unlike Equal's zero predicate.
                inverted = !inverted;
                (
                    Comparison::Equal,
                    semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64)
                        .map_err(|_| invalid())?,
                )
            };
            let branch_provenance = SelectedInstructionProvenance {
                operations: not_rows.iter().map(|row| row.operation).collect(),
                values: if not_rows.is_empty() {
                    vec![*condition]
                } else {
                    std::iter::once(base)
                        .chain(
                            not_rows
                                .iter()
                                .filter_map(|row| row.result.as_ref().map(|result| result.value)),
                        )
                        .collect()
                },
                fuel: not_rows
                    .iter()
                    .flat_map(|row| row.fuel.iter().copied())
                    .collect(),
                ..Default::default()
            };
            let when_true = successor(source, order, builder, when_true)?;
            let when_false = successor(source, order, builder, when_false)?;
            let (when_true, when_false) = if inverted {
                (when_false, when_true)
            } else {
                (when_true, when_false)
            };
            let signed = operand_type.sign() == IntegerSign::Signed;
            let kind = match predicate {
                Comparison::Equal => SelectedInstructionKind::ConditionalBranchNonZero,
                _ if signed => SelectedInstructionKind::ConditionalBranchI64LessThan,
                _ => SelectedInstructionKind::ConditionalBranchU64LessThan,
            };
            let instruction = terminal(
                builder,
                kind,
                keys.conditional_branch,
                &[],
                branch_provenance,
            )?;
            Ok(match predicate {
                Comparison::Equal => SelectedTerminator::ConditionalBranch {
                    instruction,
                    when_nonzero: when_false,
                    when_zero: when_true,
                },
                _ => {
                    let (when_less, when_not_less) = if predicate == Comparison::LessOrEqual {
                        (when_false, when_true)
                    } else {
                        (when_true, when_false)
                    };
                    if signed {
                        SelectedTerminator::ConditionalBranchI64LessThan {
                            instruction,
                            when_less,
                            when_not_less,
                        }
                    } else {
                        SelectedTerminator::ConditionalBranchU64LessThan {
                            instruction,
                            when_less,
                            when_not_less,
                        }
                    }
                }
            })
        }
    }
}

fn terminal(
    builder: &mut Builder<'_>,
    kind: SelectedInstructionKind,
    key: RegisterConstraintKey,
    operands: &[VirtualRegisterId],
    provenance: SelectedInstructionProvenance,
) -> Result<SelectedInstruction, SelectedInstructionError> {
    builder.emit(kind, key, operands, provenance)?;
    builder
        .instructions
        .last()
        .cloned()
        .ok_or(SelectedInstructionError::SourceCustodyMismatch)
}

fn successor(
    source: &LegalizedScalarFunction,
    order: &[usize],
    builder: &Builder<'_>,
    next: &LegalizedScalarSuccessor,
) -> Result<SelectedSuccessor, SelectedInstructionError> {
    let position = order
        .iter()
        .position(|index| source.blocks[*index].id == next.target)
        .ok_or(SelectedInstructionError::SourceCustodyMismatch)?;
    Ok(SelectedSuccessor {
        role: selected_instructions::SelectedSuccessorRole::Semantic,
        psi_edge: next.edge,
        block: SelectedBlockId(
            u32::try_from(position).map_err(|_| SelectedInstructionError::SourceCustodyMismatch)?,
        ),
        source_target: next.target,
        bindings: next
            .bindings
            .iter()
            .map(|semantic| {
                let transport = if source.references_value(semantic.parameter) {
                    let (_, argument, _, argument_type) = builder
                        .resolve(semantic.argument)
                        .ok_or(SelectedInstructionError::SourceCustodyMismatch)?;
                    let (_, parameter, _, parameter_type) = builder
                        .resolve(semantic.parameter)
                        .ok_or(SelectedInstructionError::SourceCustodyMismatch)?;
                    if argument_type != semantic.scalar_type
                        || parameter_type != semantic.scalar_type
                    {
                        return Err(SelectedInstructionError::SourceCustodyMismatch);
                    }
                    SelectedValueTransport::Registers {
                        argument,
                        parameter,
                    }
                } else {
                    SelectedValueTransport::Unused
                };
                Ok(SelectedValueBinding {
                    semantic: *semantic,
                    transport,
                })
            })
            .collect::<Result<Vec<_>, SelectedInstructionError>>()?,
        fuel: next.fuel.clone(),
    })
}

pub(super) fn block_order(
    source: &LegalizedScalarFunction,
) -> Result<Vec<usize>, SelectedInstructionError> {
    let invalid = SelectedInstructionError::SourceCustodyMismatch;
    let entry = source
        .blocks
        .iter()
        .position(|block| block.id == source.entry_block)
        .ok_or(invalid.clone())?;
    let mut order = vec![entry];
    if source.ranked.is_some() {
        order.extend((0..source.blocks.len()).filter(|index| *index != entry));
        return Ok(order);
    }
    while order.len() < source.blocks.len() {
        let next = source
            .blocks
            .iter()
            .enumerate()
            .position(|(index, block)| {
                !order.contains(&index)
                    && source.blocks.iter().enumerate().all(|(predecessor, row)| {
                        let targets = match &row.terminator {
                            LegalizedScalarTerminator::Return(_) => [None, None],
                            LegalizedScalarTerminator::Jump { successor, .. } => {
                                [Some(successor.target), None]
                            }
                            LegalizedScalarTerminator::Conditional {
                                when_true,
                                when_false,
                                ..
                            } => [Some(when_true.target), Some(when_false.target)],
                        };
                        !targets.contains(&Some(block.id)) || order.contains(&predecessor)
                    })
            })
            .ok_or(invalid.clone())?;
        order.push(next);
    }
    Ok(order)
}
