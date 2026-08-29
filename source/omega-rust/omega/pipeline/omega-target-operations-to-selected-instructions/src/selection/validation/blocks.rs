use crate::selection::constraints::row;
use crate::selection::shared::*;

pub(super) fn validate_selected_blocks(
    function_index: usize,
    source: &SourceFunction,
    function: &SelectedFunction,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    if function.blocks[0].source_block != source.entry_block
        || function.blocks[1].source_block != source.true_block
        || function.blocks[2].source_block != source.false_block
    {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: function
                .blocks
                .iter()
                .enumerate()
                .find(|(index, block)| {
                    block.source_block
                        != [source.entry_block, source.true_block, source.false_block][*index]
                })
                .map_or(0, |(index, _)| index as u32),
        });
    }
    let entry = &function.blocks[0];
    if entry.instructions.len() != 1 {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: 0,
        });
    }
    validate_instruction_projection(
        function_index,
        &entry.instructions[0],
        SelectedInstructionId(0),
        SelectedInstructionKind::CompareI64Zero,
        keys.compare_i64_zero,
        &[VirtualRegisterId(0)],
        &SelectedInstructionProvenance {
            values: vec![source.condition_source],
            ..Default::default()
        },
        catalog,
    )?;
    let SelectedTerminator::ConditionalBranch {
        instruction,
        when_nonzero,
        when_zero,
    } = &entry.terminator
    else {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: 0,
        });
    };
    validate_instruction_projection(
        function_index,
        instruction,
        SelectedInstructionId(1),
        SelectedInstructionKind::ConditionalBranchNonZero,
        keys.conditional_branch,
        &[],
        &SelectedInstructionProvenance {
            values: vec![source.condition_source],
            ..Default::default()
        },
        catalog,
    )?;
    let expected_true = SelectedSuccessor {
        psi_edge: source.branch_true_edge,
        block: SelectedBlockId(1),
        source_target: source.true_block,
        bindings: source.branch_true_bindings.clone(),
        fuel: source.branch_true_fuel.clone(),
    };
    let expected_false = SelectedSuccessor {
        psi_edge: source.branch_false_edge,
        block: SelectedBlockId(2),
        source_target: source.false_block,
        bindings: source.branch_false_bindings.clone(),
        fuel: source.branch_false_fuel.clone(),
    };
    if when_nonzero != &expected_true || when_zero != &expected_false {
        return Err(SelectedInstructionError::SuccessorProjectionMismatch {
            function: function_index,
            block: 0,
        });
    }
    match (&source.when_true.value, &source.when_false.value) {
        (SourceLeafValue::ActiveResidentExactAddChain(..), SourceLeafValue::Immediate { .. }) => {
            validate_active_resident_exact_add_chain_block_projection(
                function_index,
                &function.blocks[1],
                &source.when_true,
                keys,
                catalog,
            )?;
            validate_constant_return_block_projection(
                function_index,
                &function.blocks[2],
                9,
                10,
                VirtualRegisterId(7),
                &source.when_false,
                keys,
                catalog,
            )
        }
        (SourceLeafValue::Immediate { .. }, SourceLeafValue::Immediate { .. }) => {
            validate_constant_return_block_projection(
                function_index,
                &function.blocks[1],
                2,
                3,
                VirtualRegisterId(1),
                &source.when_true,
                keys,
                catalog,
            )?;
            validate_constant_return_block_projection(
                function_index,
                &function.blocks[2],
                4,
                5,
                VirtualRegisterId(2),
                &source.when_false,
                keys,
                catalog,
            )
        }
        (SourceLeafValue::EntryParameter { .. }, SourceLeafValue::EntryParameter { .. }) => {
            validate_parameter_return_block_projection(
                function_index,
                &function.blocks[1],
                2,
                VirtualRegisterId(1),
                &source.when_true,
                keys,
                catalog,
            )?;
            validate_parameter_return_block_projection(
                function_index,
                &function.blocks[2],
                3,
                VirtualRegisterId(1),
                &source.when_false,
                keys,
                catalog,
            )
        }
        (SourceLeafValue::ExactAdd { .. }, SourceLeafValue::ExactAdd { .. }) => {
            validate_exact_binary_return_block_projection(
                function_index,
                &function.blocks[1],
                [2, 3, 4, 5],
                [
                    VirtualRegisterId(1),
                    VirtualRegisterId(2),
                    VirtualRegisterId(3),
                ],
                &source.when_true,
                keys,
                catalog,
            )?;
            validate_exact_binary_return_block_projection(
                function_index,
                &function.blocks[2],
                [6, 7, 8, 9],
                [
                    VirtualRegisterId(4),
                    VirtualRegisterId(5),
                    VirtualRegisterId(6),
                ],
                &source.when_false,
                keys,
                catalog,
            )
        }
        (SourceLeafValue::WidenedExactAdd { .. }, SourceLeafValue::WidenedExactAdd { .. }) => {
            validate_exact_binary_return_block_projection(
                function_index,
                &function.blocks[1],
                [2, 3, 4, 5],
                [
                    VirtualRegisterId(1),
                    VirtualRegisterId(2),
                    VirtualRegisterId(3),
                ],
                &source.when_true,
                keys,
                catalog,
            )?;
            validate_exact_binary_return_block_projection(
                function_index,
                &function.blocks[2],
                [6, 7, 8, 9],
                [
                    VirtualRegisterId(4),
                    VirtualRegisterId(5),
                    VirtualRegisterId(6),
                ],
                &source.when_false,
                keys,
                catalog,
            )
        }
        (
            SourceLeafValue::WidenedExactSubtract { .. },
            SourceLeafValue::WidenedExactSubtract { .. },
        ) => {
            validate_exact_binary_return_block_projection(
                function_index,
                &function.blocks[1],
                [2, 3, 4, 5],
                [
                    VirtualRegisterId(1),
                    VirtualRegisterId(2),
                    VirtualRegisterId(3),
                ],
                &source.when_true,
                keys,
                catalog,
            )?;
            validate_exact_binary_return_block_projection(
                function_index,
                &function.blocks[2],
                [6, 7, 8, 9],
                [
                    VirtualRegisterId(4),
                    VirtualRegisterId(5),
                    VirtualRegisterId(6),
                ],
                &source.when_false,
                keys,
                catalog,
            )
        }
        (SourceLeafValue::ExactSubtract { .. }, SourceLeafValue::ExactSubtract { .. }) => {
            validate_exact_binary_return_block_projection(
                function_index,
                &function.blocks[1],
                [2, 3, 4, 5],
                [
                    VirtualRegisterId(1),
                    VirtualRegisterId(2),
                    VirtualRegisterId(3),
                ],
                &source.when_true,
                keys,
                catalog,
            )?;
            validate_exact_binary_return_block_projection(
                function_index,
                &function.blocks[2],
                [6, 7, 8, 9],
                [
                    VirtualRegisterId(4),
                    VirtualRegisterId(5),
                    VirtualRegisterId(6),
                ],
                &source.when_false,
                keys,
                catalog,
            )
        }
        _ => Err(SelectedInstructionError::UnsupportedSourceShape {
            function: function_index,
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_constant_return_block_projection(
    function_index: usize,
    block: &SelectedBlock,
    materialize_id: u32,
    return_id: u32,
    register: VirtualRegisterId,
    source: &SourceLeaf,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let SourceLeafValue::Immediate {
        value,
        constant_operation,
        constant_fuel,
        ..
    } = &source.value
    else {
        return Err(SelectedInstructionError::UnsupportedSourceShape {
            function: function_index,
        });
    };
    if block.instructions.len() != 1 {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    }
    validate_instruction_projection(
        function_index,
        &block.instructions[0],
        SelectedInstructionId(materialize_id),
        SelectedInstructionKind::MaterializeI64 { value: *value },
        keys.materialize_i64,
        &[register],
        &SelectedInstructionProvenance {
            operations: vec![*constant_operation],
            values: vec![source.source_value],
            fuel: constant_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )?;
    let SelectedTerminator::Return {
        instruction,
        psi_return_edge,
    } = &block.terminator
    else {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    };
    if *psi_return_edge != source.return_edge {
        return Err(SelectedInstructionError::SuccessorProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    }
    validate_instruction_projection(
        function_index,
        instruction,
        SelectedInstructionId(return_id),
        SelectedInstructionKind::ReturnI64,
        keys.return_i64,
        &[register],
        &SelectedInstructionProvenance {
            values: vec![source.source_value],
            edges: vec![source.return_edge],
            fuel: source.return_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_parameter_return_block_projection(
    function_index: usize,
    block: &SelectedBlock,
    return_id: u32,
    register: VirtualRegisterId,
    source: &SourceLeaf,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    if !matches!(source.value, SourceLeafValue::EntryParameter { .. })
        || !block.instructions.is_empty()
    {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    }
    let SelectedTerminator::Return {
        instruction,
        psi_return_edge,
    } = &block.terminator
    else {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    };
    if *psi_return_edge != source.return_edge {
        return Err(SelectedInstructionError::SuccessorProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    }
    validate_instruction_projection(
        function_index,
        instruction,
        SelectedInstructionId(return_id),
        SelectedInstructionKind::ReturnI64,
        keys.return_i64,
        &[register],
        &SelectedInstructionProvenance {
            values: vec![source.source_value],
            edges: vec![source.return_edge],
            fuel: source.return_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_exact_binary_return_block_projection(
    function_index: usize,
    block: &SelectedBlock,
    instruction_ids: [u32; 4],
    registers: [VirtualRegisterId; 3],
    source: &SourceLeaf,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let (obligation, operations, values, operation_fuel, left, right, kind, key) =
        match &source.value {
            SourceLeafValue::ExactAdd {
                obligation,
                accepted_fact,
                add_operation,
                add_fuel,
                left,
                right,
                ..
            } => (
                obligation,
                vec![*add_operation],
                vec![left.source_value, right.source_value, source.source_value],
                add_fuel.clone(),
                left,
                right,
                SelectedInstructionKind::ExactAddI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                keys.add_i64,
            ),
            SourceLeafValue::WidenedExactAdd {
                obligation,
                accepted_fact,
                add_operation,
                narrow_result,
                add_fuel,
                widen_operation,
                widen_fuel,
                left,
                right,
                ..
            } => (
                obligation,
                vec![*add_operation, *widen_operation],
                vec![
                    left.source_value,
                    right.source_value,
                    *narrow_result,
                    source.source_value,
                ],
                add_fuel.iter().chain(widen_fuel).copied().collect(),
                left,
                right,
                SelectedInstructionKind::ExactAddI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                keys.add_i64,
            ),
            SourceLeafValue::ExactSubtract {
                obligation,
                accepted_fact,
                subtract_operation,
                subtract_fuel,
                left,
                right,
                ..
            } => (
                obligation,
                vec![*subtract_operation],
                vec![left.source_value, right.source_value, source.source_value],
                subtract_fuel.clone(),
                left,
                right,
                SelectedInstructionKind::ExactSubtractI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                keys.subtract_i64,
            ),
            SourceLeafValue::WidenedExactSubtract {
                obligation,
                accepted_fact,
                subtract_operation,
                narrow_result,
                subtract_fuel,
                widen_operation,
                widen_fuel,
                left,
                right,
                ..
            } => (
                obligation,
                vec![*subtract_operation, *widen_operation],
                vec![
                    left.source_value,
                    right.source_value,
                    *narrow_result,
                    source.source_value,
                ],
                subtract_fuel.iter().chain(widen_fuel).copied().collect(),
                left,
                right,
                SelectedInstructionKind::ExactSubtractI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                keys.subtract_i64,
            ),
            _ => {
                return Err(SelectedInstructionError::UnsupportedSourceShape {
                    function: function_index,
                });
            }
        };
    if block.instructions.len() != 3 {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    }
    for (position, immediate) in [left, right].into_iter().enumerate() {
        validate_instruction_projection(
            function_index,
            &block.instructions[position],
            SelectedInstructionId(instruction_ids[position]),
            SelectedInstructionKind::MaterializeI64 {
                value: immediate.value,
            },
            keys.materialize_i64,
            &[registers[position]],
            &SelectedInstructionProvenance {
                operations: vec![immediate.constant_operation],
                values: vec![immediate.source_value],
                fuel: immediate.fuel.clone(),
                ..Default::default()
            },
            catalog,
        )?;
    }
    validate_instruction_projection(
        function_index,
        &block.instructions[2],
        SelectedInstructionId(instruction_ids[2]),
        kind,
        key,
        &registers,
        &SelectedInstructionProvenance {
            operations,
            values,
            obligations: vec![*obligation],
            fuel: operation_fuel,
            ..Default::default()
        },
        catalog,
    )?;
    let SelectedTerminator::Return {
        instruction,
        psi_return_edge,
    } = &block.terminator
    else {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    };
    if *psi_return_edge != source.return_edge {
        return Err(SelectedInstructionError::SuccessorProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    }
    validate_instruction_projection(
        function_index,
        instruction,
        SelectedInstructionId(instruction_ids[3]),
        SelectedInstructionKind::ReturnI64,
        keys.return_i64,
        &[registers[2]],
        &SelectedInstructionProvenance {
            values: vec![source.source_value],
            edges: vec![source.return_edge],
            fuel: source.return_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )
}

fn validate_active_resident_exact_add_chain_block_projection(
    function: usize,
    block: &SelectedBlock,
    source: &SourceLeaf,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let SourceLeafValue::ActiveResidentExactAddChain(chain) = &source.value else {
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    };
    if block.instructions.len() != 6 {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function,
            block: block.id.0,
        });
    }
    for (position, (id, register, immediate)) in [
        (2, VirtualRegisterId(1), &chain.resident),
        (3, VirtualRegisterId(2), &chain.left),
        (4, VirtualRegisterId(3), &chain.right),
    ]
    .into_iter()
    .enumerate()
    {
        validate_instruction_projection(
            function,
            &block.instructions[position],
            SelectedInstructionId(id),
            SelectedInstructionKind::MaterializeI64 {
                value: immediate.value,
            },
            keys.materialize_i64,
            &[register],
            &SelectedInstructionProvenance {
                operations: vec![immediate.constant_operation],
                values: vec![immediate.source_value],
                fuel: immediate.fuel.clone(),
                ..Default::default()
            },
            catalog,
        )?;
    }
    for (position, (id, registers, add, values)) in [
        (
            5,
            [
                VirtualRegisterId(2),
                VirtualRegisterId(3),
                VirtualRegisterId(4),
            ],
            &chain.inner,
            vec![
                chain.left.source_value,
                chain.right.source_value,
                chain.inner.source_value,
            ],
        ),
        (
            6,
            [
                VirtualRegisterId(1),
                VirtualRegisterId(4),
                VirtualRegisterId(5),
            ],
            &chain.middle,
            vec![
                chain.resident.source_value,
                chain.inner.source_value,
                chain.middle.source_value,
            ],
        ),
        (
            7,
            [
                VirtualRegisterId(1),
                VirtualRegisterId(5),
                VirtualRegisterId(6),
            ],
            &chain.result,
            vec![
                chain.resident.source_value,
                chain.middle.source_value,
                chain.result.source_value,
            ],
        ),
    ]
    .into_iter()
    .enumerate()
    {
        validate_instruction_projection(
            function,
            &block.instructions[position + 3],
            SelectedInstructionId(id),
            SelectedInstructionKind::ExactAddI64 {
                obligation: add.obligation,
                accepted_fact: add.accepted_fact,
            },
            keys.add_i64,
            &registers,
            &SelectedInstructionProvenance {
                operations: vec![add.operation],
                values,
                obligations: vec![add.obligation],
                fuel: add.fuel.clone(),
                ..Default::default()
            },
            catalog,
        )?;
    }
    let SelectedTerminator::Return {
        instruction,
        psi_return_edge,
    } = &block.terminator
    else {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function,
            block: block.id.0,
        });
    };
    if *psi_return_edge != source.return_edge {
        return Err(SelectedInstructionError::SuccessorProjectionMismatch {
            function,
            block: block.id.0,
        });
    }
    validate_instruction_projection(
        function,
        instruction,
        SelectedInstructionId(8),
        SelectedInstructionKind::ReturnI64,
        keys.return_i64,
        &[VirtualRegisterId(6)],
        &SelectedInstructionProvenance {
            values: vec![source.source_value],
            edges: vec![source.return_edge],
            fuel: source.return_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_instruction_projection(
    function: usize,
    instruction: &SelectedInstruction,
    id: SelectedInstructionId,
    kind: SelectedInstructionKind,
    key: RegisterConstraintKey,
    registers: &[VirtualRegisterId],
    provenance: &SelectedInstructionProvenance,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let constraint = row(catalog, key)?;
    if instruction.id != id
        || instruction.kind != kind
        || instruction.constraint != key
        || instruction.provenance != *provenance
        || instruction.operands.len() != registers.len()
        || instruction
            .operands
            .iter()
            .zip(registers)
            .zip(&constraint.operands)
            .any(|((operand, register), expected)| {
                operand.virtual_register != *register
                    || operand.operand != expected.operand
                    || operand.access != expected.access
                    || operand.class != expected.class
                    || operand.fixed_view != expected.fixed_view
                    || operand.tied_to != expected.tied_to
                    || operand.early_clobber != expected.early_clobber
            })
    {
        return Err(SelectedInstructionError::InstructionProjectionMismatch {
            function,
            instruction: id.0,
        });
    }
    if instruction.implicit_uses != constraint.implicit_uses
        || instruction.implicit_defs != constraint.implicit_defs
        || instruction.clobbers != constraint.clobbers
    {
        return Err(SelectedInstructionError::ConstraintEffectMismatch {
            function,
            instruction: id.0,
        });
    }
    Ok(())
}
