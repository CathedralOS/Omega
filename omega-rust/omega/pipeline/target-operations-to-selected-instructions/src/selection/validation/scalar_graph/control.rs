//! Independent control and canonical block-order replay.
use super::*;
use legalized_operations::{
    LegalizedScalarBlock, LegalizedScalarComparison as Comparison, LegalizedScalarReturnValue,
    LegalizedScalarSuccessor, LegalizedScalarTerminator,
};
use selected_instructions::SelectedValueTransport;

pub(super) fn validate(
    source: &LegalizedScalarFunction,
    block: &LegalizedScalarBlock,
    replay: &mut Replay<'_>,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let function = replay.function;
    let invalid = || SelectedInstructionError::FunctionProjectionMismatch { function };
    let constraints = replay.constraints;
    let keys = &constraints.keys;
    let selected_block = replay.block;
    let (actual, kind, key, operands, provenance) =
        match (&block.terminator, &selected_block.terminator) {
            (
                LegalizedScalarTerminator::Return(returned),
                SelectedTerminator::Return {
                    instruction,
                    psi_return_edge,
                },
            ) => {
                if *psi_return_edge != returned.edge {
                    return Err(invalid());
                }
                let (kind, key, operands, values) = match returned.value {
                    LegalizedScalarReturnValue::Unit => (
                        SelectedInstructionKind::ReturnUnit,
                        keys.return_unit,
                        Vec::new(),
                        Vec::new(),
                    ),
                    LegalizedScalarReturnValue::Value { value, scalar_type } => {
                        let (_, input, site, value_type) =
                            replay.resolve(value).ok_or_else(invalid)?;
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
                        let [operand] = row(catalog, keys.return_i64)?.operands.as_slice() else {
                            return Err(invalid());
                        };
                        if operand.fixed_view.is_none()
                            || operand.fixed_view != environment.fixed_register_view(*register)
                        {
                            return Err(invalid());
                        }
                        let key = keys.return_i64;
                        let output = replay.check_copy(input, value, site, value_type)?;
                        (
                            SelectedInstructionKind::ReturnI64,
                            key,
                            vec![output],
                            vec![value],
                        )
                    }
                };
                (
                    instruction,
                    kind,
                    key,
                    operands,
                    SelectedInstructionProvenance {
                        values,
                        edges: vec![returned.edge],
                        fuel: returned.fuel.clone(),
                        ..Default::default()
                    },
                )
            }
            (
                LegalizedScalarTerminator::Jump { successor, .. },
                SelectedTerminator::Jump {
                    instruction,
                    successor: actual,
                },
            ) => {
                check_successor(replay, successor, actual)?;
                (
                    instruction,
                    SelectedInstructionKind::Jump,
                    keys.jump,
                    Vec::new(),
                    Default::default(),
                )
            }
            (
                LegalizedScalarTerminator::Conditional {
                    condition,
                    when_true,
                    when_false,
                    ..
                },
                actual,
            ) => {
                let Some(comparison) = block.instructions.last() else {
                    return Err(invalid());
                };
                let LegalizedScalarInstructionKind::Compare {
                    predicate,
                    operand_type,
                    ..
                } = comparison.kind
                else {
                    return Err(invalid());
                };
                if comparison.result != *condition {
                    return Err(invalid());
                }
                let (instruction, actual_true, actual_false, kind) =
                    match (predicate, operand_type.sign(), actual) {
                        (
                            Comparison::Equal,
                            _,
                            SelectedTerminator::ConditionalBranch {
                                instruction,
                                when_nonzero,
                                when_zero,
                            },
                        ) => (
                            instruction,
                            when_zero,
                            when_nonzero,
                            SelectedInstructionKind::ConditionalBranchNonZero,
                        ),
                        (
                            Comparison::LessThan,
                            IntegerSign::Signed,
                            SelectedTerminator::ConditionalBranchI64LessThan {
                                instruction,
                                when_less,
                                when_not_less,
                            },
                        ) => (
                            instruction,
                            when_less,
                            when_not_less,
                            SelectedInstructionKind::ConditionalBranchI64LessThan,
                        ),
                        (
                            Comparison::LessOrEqual,
                            IntegerSign::Signed,
                            SelectedTerminator::ConditionalBranchI64LessThan {
                                instruction,
                                when_less,
                                when_not_less,
                            },
                        ) => (
                            instruction,
                            when_not_less,
                            when_less,
                            SelectedInstructionKind::ConditionalBranchI64LessThan,
                        ),
                        (
                            Comparison::LessThan,
                            IntegerSign::Unsigned,
                            SelectedTerminator::ConditionalBranchU64LessThan {
                                instruction,
                                when_less,
                                when_not_less,
                            },
                        ) => (
                            instruction,
                            when_less,
                            when_not_less,
                            SelectedInstructionKind::ConditionalBranchU64LessThan,
                        ),
                        (
                            Comparison::LessOrEqual,
                            IntegerSign::Unsigned,
                            SelectedTerminator::ConditionalBranchU64LessThan {
                                instruction,
                                when_less,
                                when_not_less,
                            },
                        ) => (
                            instruction,
                            when_not_less,
                            when_less,
                            SelectedInstructionKind::ConditionalBranchU64LessThan,
                        ),
                        _ => return Err(invalid()),
                    };
                check_successor(replay, when_true, actual_true)?;
                check_successor(replay, when_false, actual_false)?;
                (
                    instruction,
                    kind,
                    keys.conditional_branch,
                    Vec::new(),
                    SelectedInstructionProvenance {
                        values: vec![*condition],
                        ..Default::default()
                    },
                )
            }
            _ => return Err(invalid()),
        };
    if actual.id.0 as usize != replay.instruction_cursor
        || actual.kind != kind
        || actual.constraint != key
        || actual.provenance != provenance
        || actual
            .operands
            .iter()
            .map(|operand| operand.virtual_register)
            .ne(operands)
    {
        return Err(invalid());
    }
    Ok(())
}

fn check_successor(
    replay: &Replay<'_>,
    source: &LegalizedScalarSuccessor,
    actual: &SelectedSuccessor,
) -> Result<(), SelectedInstructionError> {
    let matches = replay
        .selected
        .blocks
        .iter()
        .filter(|block| block.source_block == source.target)
        .collect::<Vec<_>>();
    let [block] = matches.as_slice() else {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    };
    if actual.psi_edge != source.edge
        || actual.block != block.id
        || actual.source_target != source.target
        || actual.bindings.len() != source.bindings.len()
        || actual.fuel != source.fuel
    {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    }
    for (actual, semantic) in actual.bindings.iter().zip(&source.bindings) {
        if actual.semantic != *semantic {
            return Err(SelectedInstructionError::SourceCustodyMismatch);
        }
        let destination = replay.selected.virtual_registers.iter().find(|register| {
            matches!(register.origin,
            VirtualRegisterOrigin::BlockParameter {source_value,block:owner,..}
            if source_value==semantic.parameter && owner==block.id)
        });
        match (actual.transport, destination) {
            (SelectedValueTransport::Unused, None) => {}
            (
                SelectedValueTransport::Registers {
                    argument,
                    parameter,
                },
                Some(destination),
            ) => {
                let (_, expected, _, scalar_type) = replay
                    .resolve(semantic.argument)
                    .ok_or(SelectedInstructionError::SourceCustodyMismatch)?;
                if argument != expected
                    || parameter != destination.id
                    || scalar_type != semantic.scalar_type
                    || destination.scalar_type != semantic.scalar_type
                {
                    return Err(SelectedInstructionError::SourceCustodyMismatch);
                }
            }
            _ => return Err(SelectedInstructionError::SourceCustodyMismatch),
        }
    }
    Ok(())
}

pub(super) fn block_order(
    source: &LegalizedScalarFunction,
    selected: &SelectedFunction,
) -> Result<(), SelectedInstructionError> {
    let invalid = SelectedInstructionError::SourceCustodyMismatch;
    if source.blocks.len() != selected.blocks.len() || selected.blocks.is_empty() {
        return Err(invalid);
    }
    let mut seen = Vec::new();
    for (position, actual) in selected.blocks.iter().enumerate() {
        if actual.id.0 as usize != position || seen.contains(&actual.source_block) {
            return Err(invalid);
        }
        let expected = if position == 0 {
            source.entry_block
        } else {
            source
                .blocks
                .iter()
                .find(|candidate| {
                    !seen.contains(&candidate.id)
                        && source.blocks.iter().all(|predecessor| {
                            let names_candidate = match &predecessor.terminator {
                                LegalizedScalarTerminator::Return(_) => false,
                                LegalizedScalarTerminator::Jump { successor, .. } => {
                                    successor.target == candidate.id
                                }
                                LegalizedScalarTerminator::Conditional {
                                    when_true,
                                    when_false,
                                    ..
                                } => {
                                    when_true.target == candidate.id
                                        || when_false.target == candidate.id
                                }
                            };
                            !names_candidate || seen.contains(&predecessor.id)
                        })
                })
                .map(|block| block.id)
                .ok_or(invalid.clone())?
        };
        if actual.source_block != expected
            || !source
                .blocks
                .iter()
                .any(|block| block.id == actual.source_block)
        {
            return Err(invalid);
        }
        seen.push(actual.source_block);
    }
    Ok(())
}
