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
    if matches!(
        block.terminator,
        LegalizedScalarTerminator::StructuralCase { .. }
    ) {
        return super::structural_case::validate(block, replay, catalog);
    }
    if super::process_exit::validate(block, replay)? {
        return Ok(());
    }
    let function = replay.function;
    let invalid = || SelectedInstructionError::FunctionProjectionMismatch { function };
    let constraints = replay.constraints;
    let keys = &constraints.keys;
    let selected_block = replay.block;
    let (actual, kind, key, operands, provenance) = match (
        &block.terminator,
        &selected_block.terminator,
    ) {
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
                LegalizedScalarReturnValue::Structural { .. }
                | LegalizedScalarReturnValue::StructuralParameter { .. } => {
                    return super::aggregate_return::validate(
                        source,
                        block,
                        returned,
                        replay,
                        environment,
                        catalog,
                    );
                }
                LegalizedScalarReturnValue::Unit => (
                    SelectedInstructionKind::ReturnUnit,
                    keys.return_unit,
                    Vec::new(),
                    Vec::new(),
                ),
                LegalizedScalarReturnValue::Value { value, scalar_type } => {
                    let (_, input, site, value_type) = replay.resolve(value).ok_or_else(invalid)?;
                    if value_type != scalar_type {
                        return Err(invalid());
                    }
                    let result = source.call_plan.result.as_ref().ok_or_else(invalid)?;
                    let [
                        ValueLocation::Register {
                            register,
                            value_byte_offset: 0,
                            byte_size,
                        },
                    ] = result.locations.as_slice()
                    else {
                        return Err(invalid());
                    };
                    if crate::selection::scalar_call_abi::scalar_shape(value_type)
                        != Some(result.shape)
                        || *byte_size != result.shape.byte_size
                    {
                        return Err(invalid());
                    }
                    let keys_for_result = if matches!(scalar_type, ScalarType::IeeeFloat(_)) {
                        keys.return_float.as_slice()
                    } else {
                        std::slice::from_ref(&keys.return_i64)
                    };
                    let key = keys_for_result.iter().copied().find(|key| row(catalog, *key).is_ok_and(|row| matches!(row.operands.as_slice(), [operand] if operand.fixed_view.is_some() && operand.fixed_view == environment.fixed_register_view(*register)))).ok_or_else(invalid)?;
                    let output = if let Some((kind, transfer_key)) =
                        crate::selection::scalar_call_abi::outgoing_float_transfer(value_type, keys)
                    {
                        let class = row(catalog, transfer_key)?
                            .operands
                            .get(1)
                            .ok_or_else(invalid)?
                            .class;
                        let output = replay.check_register_class(
                            class,
                            site,
                            value_type,
                            VirtualRegisterOrigin::InstructionResult {
                                instruction: SelectedInstructionId(
                                    replay
                                        .instruction_cursor
                                        .try_into()
                                        .map_err(|_| invalid())?,
                                ),
                                source_value: value,
                            },
                            None,
                        )?;
                        replay.check_instruction(
                            kind,
                            transfer_key,
                            &[input, output],
                            &SelectedInstructionProvenance {
                                values: vec![value],
                                ..Default::default()
                            },
                        )?;
                        output
                    } else {
                        replay.check_copy(input, value, site, value_type)?
                    };
                    (
                        SelectedInstructionKind::ReturnScalar,
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
            check_successor(source, replay, successor, actual)?;
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
            let mut base = *condition;
            let mut suffix_start = block.instructions.len();
            let mut inverted = false;
            let mut not_rows = Vec::new();
            while replay.resolve(base).is_none()
                && let Some(previous) = suffix_start
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
                    if replay.resolve(base).is_some() {
                        return false;
                    }
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
                let (_, input, _, scalar_type) = replay.resolve(base).ok_or_else(invalid)?;
                if scalar_type != ScalarType::Boolean {
                    return Err(invalid());
                }
                replay.check_instruction(
                    SelectedInstructionKind::CompareI64Zero,
                    keys.compare_i64_zero,
                    &[input],
                    &SelectedInstructionProvenance {
                        values: vec![base],
                        ..Default::default()
                    },
                )?;
                // A Boolean register is true when nonzero, unlike Equal's zero predicate.
                inverted = !inverted;
                (Comparison::Equal, ScalarType::Boolean)
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
            let comparison_sign = match operand_type {
                ScalarType::Integer(integer) => integer.sign(),
                ScalarType::Boolean => IntegerSign::Unsigned,
                ScalarType::IeeeFloat(_) => return Err(invalid()),
            };
            let (instruction, actual_true, actual_false, kind) =
                match (predicate, comparison_sign, actual) {
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
            let (actual_true, actual_false) = if inverted {
                (actual_false, actual_true)
            } else {
                (actual_true, actual_false)
            };
            check_successor(source, replay, when_true, actual_true)?;
            check_successor(source, replay, when_false, actual_false)?;
            (
                instruction,
                kind,
                keys.conditional_branch,
                Vec::new(),
                branch_provenance,
            )
        }
        _ => return Err(invalid()),
    };
    let provenance = replay.settle_provenance(provenance);
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
    function: &LegalizedScalarFunction,
    replay: &Replay<'_>,
    source: &LegalizedScalarSuccessor,
    actual: &SelectedSuccessor,
) -> Result<(), SelectedInstructionError> {
    if actual.structural_case.is_some() {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    }
    let matches = replay
        .selected
        .blocks
        .iter()
        .filter(|block| {
            block.origin == selected_instructions::SelectedBlockOrigin::Source(source.target)
        })
        .collect::<Vec<_>>();
    let [block] = matches.as_slice() else {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    };
    if actual.psi_edge != source.edge
        || actual.block != block.id
        || actual.source_target != source.target
        || actual.bindings.len() != source.bindings.len()
        || actual.structural_bindings.len() != source.structural_bindings.len()
        || actual.fuel != source.fuel
    {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    }
    for (actual, semantic) in actual
        .structural_bindings
        .iter()
        .zip(&source.structural_bindings)
    {
        if semantic.argument.access == terminal_psi::StructuralAccess::Owned
            && crate::unobserved_owned_input::accepts(function)
        {
            if actual.semantic != *semantic
                || actual.transport != selected_instructions::SelectedStructuralTransport::Unused
            {
                return Err(SelectedInstructionError::SourceCustodyMismatch);
            }
            continue;
        }
        let pointer = replay
            .transport
            .pointers
            .iter()
            .find(|(place, _)| *place == semantic.argument.place)
            .map(|(_, pointer)| *pointer)
            .ok_or(SelectedInstructionError::SourceCustodyMismatch)?;
        let destination = selected_instructions::LocalStorageSlotId::StructuralBlockParameter {
            block: source.target,
            place: semantic.parameter,
        };
        let expected = if semantic.argument.access == terminal_psi::StructuralAccess::Owned {
            let parameter = function
                .blocks
                .iter()
                .find(|block| block.id == source.target)
                .ok_or(SelectedInstructionError::SourceCustodyMismatch)?
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == semantic.parameter)
                .ok_or(SelectedInstructionError::SourceCustodyMismatch)?;
            let shape = crate::selection::aggregate_result_input::block_parameter_shape(
                function, parameter,
            )
            .ok_or(SelectedInstructionError::SourceCustodyMismatch)?;
            selected_instructions::SelectedStructuralTransport::WholeValue {
                argument: pointer,
                destination,
                byte_size: shape.byte_size,
                alignment: shape.alignment,
            }
        } else {
            selected_instructions::SelectedStructuralTransport::Descriptor {
                argument: pointer,
                destination,
            }
        };
        if actual.semantic != *semantic || actual.transport != expected {
            return Err(SelectedInstructionError::SourceCustodyMismatch);
        }
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
    if source.blocks.len() > selected.blocks.len() || selected.blocks.is_empty() {
        return Err(invalid);
    }
    let order = crate::selection::block_order::derive(source)?;
    for (position, actual) in selected.blocks.iter().take(source.blocks.len()).enumerate() {
        if actual.id.0 as usize != position {
            return Err(invalid);
        }
        if actual.origin
            != selected_instructions::SelectedBlockOrigin::Source(source.blocks[order[position]].id)
        {
            return Err(invalid);
        }
    }
    let mut extra = source.blocks.len();
    for source_position in order {
        let block = &source.blocks[source_position];
        if let legalized_operations::LegalizedScalarTerminator::StructuralCase { cases, .. } =
            &block.terminator
        {
            for ordinal in 1..cases.len().saturating_sub(1) {
                let actual = selected.blocks.get(extra).ok_or(invalid.clone())?;
                if actual.id.0 as usize != extra
                    || actual.origin
                        != (selected_instructions::SelectedBlockOrigin::CaseDispatch {
                            source: block.id,
                            case_ordinal: ordinal.try_into().map_err(|_| invalid.clone())?,
                        })
                {
                    return Err(invalid);
                }
                extra += 1;
            }
        }
    }
    if extra != selected.blocks.len() {
        return Err(invalid);
    }
    Ok(())
}

pub(super) fn branch_suffix(
    source: &LegalizedScalarFunction,
    block: &legalized_operations::LegalizedScalarBlock,
    index: usize,
) -> bool {
    let Some(result) = block.instructions.get(index).and_then(|row| row.result) else {
        return false;
    };
    let mut value = result.value;
    for row in &block.instructions[index + 1..] {
        if !matches!(row.kind, LegalizedScalarInstructionKind::BooleanNot {operand} if operand == value)
            || row
                .result
                .is_none_or(|result| result.scalar_type != ScalarType::Boolean)
        {
            return false;
        }
        let Some(result) = row.result else {
            return false;
        };
        value = result.value;
    }

    for (position, row) in block.instructions.iter().enumerate().skip(index) {
        let Some(result) = row.result else {
            return false;
        };
        for owner in &source.blocks {
            for (consumer_index, consumer) in owner.instructions.iter().enumerate() {
                if owner.id == block.id && consumer_index == position + 1 {
                    continue;
                }
                if consumer.references_value(result.value) {
                    return false;
                }
            }
            if owner.id == block.id && position + 1 == block.instructions.len() {
                let legalized_operations::LegalizedScalarTerminator::Conditional {
                    condition,
                    when_true,
                    when_false,
                    ..
                } = &owner.terminator
                else {
                    return false;
                };
                if *condition != result.value
                    || [when_true, when_false]
                        .into_iter()
                        .flat_map(|edge| &edge.bindings)
                        .any(|binding| binding.argument == result.value)
                {
                    return false;
                }
            } else if owner.terminator.references_value(result.value) {
                return false;
            }
        }
    }
    matches!(block.terminator, legalized_operations::LegalizedScalarTerminator::Conditional {condition,..}
        if condition == value)
}
