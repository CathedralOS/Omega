use crate::selection::constraints::row;
use crate::selection::shared::*;

pub(super) fn validate_dense(
    function_index: usize,
    source: &SourceFunction,
    function: &SelectedFunction,
) -> Result<(), SelectedInstructionError> {
    let (expected_register_count, expected_instruction_count) =
        match (&source.when_true.value, &source.when_false.value) {
            (SourceLeafValue::Immediate { .. }, SourceLeafValue::Immediate { .. }) => (
                if matches!(
                    source.condition,
                    LegalizedCondition::IntegerEqualParametersV1 { .. }
                        | LegalizedCondition::IntegerLessThanParametersV1 { .. }
                        | LegalizedCondition::IntegerLessOrEqualParametersV1 { .. }
                        | LegalizedCondition::IntegerNotEqualParametersV1 { .. }
                        | LegalizedCondition::I64LessThanParametersV1 { .. }
                ) {
                    4
                } else {
                    3
                },
                6,
            ),
            (
                SourceLeafValue::ActiveResidentExactAddChain(..),
                SourceLeafValue::Immediate { .. },
            ) => (8, 11),
            (
                SourceLeafValue::ActiveResidentExactAddBridgeChain(..),
                SourceLeafValue::Immediate { .. },
            ) => (9, 12),
            (
                SourceLeafValue::ActiveResidentExactAddOriginalVictimChain(..),
                SourceLeafValue::Immediate { .. },
            ) => (10, 13),
            (SourceLeafValue::EntryParameter { .. }, SourceLeafValue::EntryParameter { .. }) => {
                (2, 4)
            }
            (SourceLeafValue::ExactAdd { .. }, SourceLeafValue::ExactAdd { .. }) => (7, 10),
            (SourceLeafValue::WidenedExactAdd { .. }, SourceLeafValue::WidenedExactAdd { .. }) => {
                (7, 10)
            }
            (
                SourceLeafValue::WidenedExactSubtract { .. },
                SourceLeafValue::WidenedExactSubtract { .. },
            ) => (7, 10),
            (SourceLeafValue::ExactSubtract { .. }, SourceLeafValue::ExactSubtract { .. }) => {
                (7, 10)
            }
            _ => {
                return Err(SelectedInstructionError::UnsupportedSourceShape {
                    function: function_index,
                });
            }
        };
    if function.virtual_registers.len() != expected_register_count
        || function
            .virtual_registers
            .iter()
            .enumerate()
            .any(|(index, register)| register.id.0 as usize != index)
    {
        return Err(SelectedInstructionError::NonCanonicalVirtualRegisters {
            function: function_index,
        });
    }
    if function.blocks.len() != 3
        || function
            .blocks
            .iter()
            .enumerate()
            .any(|(index, block)| block.id.0 as usize != index)
    {
        return Err(SelectedInstructionError::NonCanonicalBlocks {
            function: function_index,
        });
    }
    let mut ids = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .chain(std::iter::once(
                    terminator_instruction(&block.terminator).id,
                ))
        })
        .collect::<Vec<_>>();
    ids.sort_unstable();
    if ids
        != (0..expected_instruction_count)
            .map(SelectedInstructionId)
            .collect::<Vec<_>>()
    {
        return Err(SelectedInstructionError::NonCanonicalInstructions {
            function: function_index,
        });
    }
    Ok(())
}

pub(super) fn validate_block_constraints(
    function_index: usize,
    block: &SelectedBlock,
    function: &SelectedFunction,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    for instruction in block
        .instructions
        .iter()
        .chain(std::iter::once(terminator_instruction(&block.terminator)))
    {
        let row = row(catalog, instruction.constraint)?;
        if instruction.operands.len() != row.operands.len() {
            return Err(SelectedInstructionError::ConstraintOperandMismatch {
                function: function_index,
                instruction: instruction.id.0,
            });
        }
        for (operand, constraint) in instruction.operands.iter().zip(&row.operands) {
            let Some(register) = function
                .virtual_registers
                .get(operand.virtual_register.0 as usize)
            else {
                return Err(SelectedInstructionError::ConstraintOperandMismatch {
                    function: function_index,
                    instruction: instruction.id.0,
                });
            };
            if operand.operand != constraint.operand
                || operand.access != constraint.access
                || operand.class != constraint.class
                || operand.fixed_view != constraint.fixed_view
                || operand.tied_to != constraint.tied_to
                || operand.early_clobber != constraint.early_clobber
                || register.class != constraint.class
            {
                return Err(SelectedInstructionError::ConstraintOperandMismatch {
                    function: function_index,
                    instruction: instruction.id.0,
                });
            }
        }
        if instruction.implicit_uses != row.implicit_uses
            || instruction.implicit_defs != row.implicit_defs
            || instruction.clobbers != row.clobbers
        {
            return Err(SelectedInstructionError::ConstraintEffectMismatch {
                function: function_index,
                instruction: instruction.id.0,
            });
        }
    }
    Ok(())
}

pub(super) fn validate_def_use(
    function_index: usize,
    function: &SelectedFunction,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let mut definitions = vec![0_u8; function.virtual_registers.len()];
    let entry_registers = function
        .virtual_registers
        .iter()
        .filter_map(|register| {
            matches!(
                register.origin,
                VirtualRegisterOrigin::EntryParameter { .. }
            )
            .then_some(register.id)
        })
        .collect::<BTreeSet<_>>();
    for register in &entry_registers {
        definitions[register.0 as usize] = 1;
    }
    for block in &function.blocks {
        let mut available = entry_registers.clone();
        for instruction in block
            .instructions
            .iter()
            .chain(std::iter::once(terminator_instruction(&block.terminator)))
        {
            let row = row(catalog, instruction.constraint)?;
            for (operand, constraint) in instruction.operands.iter().zip(&row.operands) {
                let index = operand.virtual_register.0 as usize;
                if matches!(
                    constraint.access,
                    RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
                ) && !available.contains(&operand.virtual_register)
                {
                    return Err(SelectedInstructionError::UseBeforeDefinition {
                        function: function_index,
                        instruction: instruction.id.0,
                        register: operand.virtual_register.0,
                    });
                }
                if matches!(
                    constraint.access,
                    RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                ) {
                    definitions[index] += 1;
                    if definitions[index] != 1 {
                        return Err(SelectedInstructionError::MultipleDefinitions {
                            function: function_index,
                            register: operand.virtual_register.0,
                        });
                    }
                    available.insert(operand.virtual_register);
                }
            }
        }
    }
    if definitions.iter().any(|count| *count != 1) {
        return Err(SelectedInstructionError::MultipleDefinitions {
            function: function_index,
            register: definitions.iter().position(|count| *count != 1).unwrap() as u32,
        });
    }
    Ok(())
}

pub(super) fn validate_provenance_partition(
    function_index: usize,
    source: &SourceFunction,
    function: &SelectedFunction,
) -> Result<(), SelectedInstructionError> {
    let entry = &function.blocks[0];
    let (
        branch,
        first_successor,
        second_successor,
        expected_compare_fuel,
        expected_branch_fuel,
        expected_first_fuel,
        expected_second_fuel,
    ) = match (&source.condition, &entry.terminator) {
        (
            LegalizedCondition::DirectParameter { .. },
            SelectedTerminator::ConditionalBranch {
                instruction,
                when_nonzero,
                when_zero,
            },
        ) => (
            instruction,
            when_nonzero,
            when_zero,
            &[][..],
            &[][..],
            source.branch_true_fuel.as_slice(),
            source.branch_false_fuel.as_slice(),
        ),
        (
            LegalizedCondition::IntegerEqualParametersV1 { fuel, .. },
            SelectedTerminator::ConditionalBranch {
                instruction,
                when_nonzero,
                when_zero,
            },
        ) => (
            instruction,
            when_nonzero,
            when_zero,
            fuel.as_slice(),
            &[][..],
            source.branch_false_fuel.as_slice(),
            source.branch_true_fuel.as_slice(),
        ),
        (
            LegalizedCondition::IntegerNotEqualParametersV1 {
                equality_fuel,
                boolean_not_fuel,
                ..
            },
            SelectedTerminator::ConditionalBranch {
                instruction,
                when_nonzero,
                when_zero,
            },
        ) => (
            instruction,
            when_nonzero,
            when_zero,
            equality_fuel.as_slice(),
            boolean_not_fuel.as_slice(),
            source.branch_true_fuel.as_slice(),
            source.branch_false_fuel.as_slice(),
        ),
        (
            LegalizedCondition::IntegerLessThanParametersV1 { fuel, .. },
            SelectedTerminator::ConditionalBranchU64LessThan {
                instruction,
                when_less,
                when_not_less,
            },
        ) => (
            instruction,
            when_less,
            when_not_less,
            fuel.as_slice(),
            &[][..],
            source.branch_true_fuel.as_slice(),
            source.branch_false_fuel.as_slice(),
        ),
        (
            LegalizedCondition::IntegerLessOrEqualParametersV1 { fuel, .. },
            SelectedTerminator::ConditionalBranchU64LessThan {
                instruction,
                when_less,
                when_not_less,
            },
        ) => (
            instruction,
            when_less,
            when_not_less,
            fuel.as_slice(),
            &[][..],
            source.branch_false_fuel.as_slice(),
            source.branch_true_fuel.as_slice(),
        ),
        (
            LegalizedCondition::I64LessThanParametersV1 { fuel, .. },
            SelectedTerminator::ConditionalBranchI64LessThan {
                instruction,
                when_less,
                when_not_less,
            },
        ) => (
            instruction,
            when_less,
            when_not_less,
            fuel.as_slice(),
            &[][..],
            source.branch_true_fuel.as_slice(),
            source.branch_false_fuel.as_slice(),
        ),
        _ => {
            return Err(SelectedInstructionError::ProvenancePartitionMismatch {
                function: function_index,
            });
        }
    };
    if entry.instructions[0].provenance.fuel != expected_compare_fuel
        || branch.provenance.fuel != expected_branch_fuel
        || first_successor.fuel != expected_first_fuel
        || second_successor.fuel != expected_second_fuel
    {
        return Err(SelectedInstructionError::ProvenancePartitionMismatch {
            function: function_index,
        });
    }
    for (block, leaf) in function.blocks[1..]
        .iter()
        .zip([&source.when_true, &source.when_false])
    {
        let SelectedTerminator::Return { instruction, .. } = &block.terminator else {
            return Err(SelectedInstructionError::ProvenancePartitionMismatch {
                function: function_index,
            });
        };
        match &leaf.value {
            SourceLeafValue::Immediate { constant_fuel, .. } => {
                if block.instructions.len() != 1
                    || block.instructions[0].provenance.fuel != *constant_fuel
                    || instruction.provenance.fuel != leaf.return_fuel
                {
                    return Err(SelectedInstructionError::ProvenancePartitionMismatch {
                        function: function_index,
                    });
                }
            }
            SourceLeafValue::EntryParameter { .. } => {
                if !block.instructions.is_empty() || instruction.provenance.fuel != leaf.return_fuel
                {
                    return Err(SelectedInstructionError::ProvenancePartitionMismatch {
                        function: function_index,
                    });
                }
            }
            SourceLeafValue::ExactAdd {
                add_fuel,
                left,
                right,
                ..
            } => {
                if block.instructions.len() != 3
                    || block.instructions[0].provenance.fuel != left.fuel
                    || block.instructions[1].provenance.fuel != right.fuel
                    || block.instructions[2].provenance.fuel != *add_fuel
                    || instruction.provenance.fuel != leaf.return_fuel
                {
                    return Err(SelectedInstructionError::ProvenancePartitionMismatch {
                        function: function_index,
                    });
                }
            }
            SourceLeafValue::WidenedExactAdd {
                add_fuel,
                widen_fuel,
                left,
                right,
                ..
            } => {
                let legal_fuel = add_fuel
                    .iter()
                    .chain(widen_fuel)
                    .copied()
                    .collect::<Vec<_>>();
                if block.instructions.len() != 3
                    || block.instructions[0].provenance.fuel != left.fuel
                    || block.instructions[1].provenance.fuel != right.fuel
                    || block.instructions[2].provenance.fuel != legal_fuel
                    || instruction.provenance.fuel != leaf.return_fuel
                {
                    return Err(SelectedInstructionError::ProvenancePartitionMismatch {
                        function: function_index,
                    });
                }
            }
            SourceLeafValue::WidenedExactSubtract {
                subtract_fuel,
                widen_fuel,
                left,
                right,
                ..
            } => {
                let legal_fuel = subtract_fuel
                    .iter()
                    .chain(widen_fuel)
                    .copied()
                    .collect::<Vec<_>>();
                if block.instructions.len() != 3
                    || block.instructions[0].provenance.fuel != left.fuel
                    || block.instructions[1].provenance.fuel != right.fuel
                    || block.instructions[2].provenance.fuel != legal_fuel
                    || instruction.provenance.fuel != leaf.return_fuel
                {
                    return Err(SelectedInstructionError::ProvenancePartitionMismatch {
                        function: function_index,
                    });
                }
            }
            SourceLeafValue::ExactSubtract {
                subtract_fuel,
                left,
                right,
                ..
            } => {
                if block.instructions.len() != 3
                    || block.instructions[0].provenance.fuel != left.fuel
                    || block.instructions[1].provenance.fuel != right.fuel
                    || block.instructions[2].provenance.fuel != *subtract_fuel
                    || instruction.provenance.fuel != leaf.return_fuel
                {
                    return Err(SelectedInstructionError::ProvenancePartitionMismatch {
                        function: function_index,
                    });
                }
            }
            SourceLeafValue::ActiveResidentExactAddChain(chain) => {
                if block.instructions.len() != 6
                    || block.instructions[0].provenance.fuel != chain.resident.fuel
                    || block.instructions[1].provenance.fuel != chain.left.fuel
                    || block.instructions[2].provenance.fuel != chain.right.fuel
                    || block.instructions[3].provenance.fuel != chain.inner.fuel
                    || block.instructions[4].provenance.fuel != chain.middle.fuel
                    || block.instructions[5].provenance.fuel != chain.result.fuel
                    || instruction.provenance.fuel != leaf.return_fuel
                {
                    return Err(SelectedInstructionError::ProvenancePartitionMismatch {
                        function: function_index,
                    });
                }
            }
            SourceLeafValue::ActiveResidentExactAddBridgeChain(chain) => {
                if block.instructions.len() != 7
                    || block.instructions[0].provenance.fuel != chain.resident.fuel
                    || block.instructions[1].provenance.fuel != chain.left.fuel
                    || block.instructions[2].provenance.fuel != chain.right.fuel
                    || block.instructions[3].provenance.fuel != chain.inner.fuel
                    || block.instructions[4].provenance.fuel != chain.middle.fuel
                    || block.instructions[5].provenance.fuel != chain.bridge.fuel
                    || block.instructions[6].provenance.fuel != chain.result.fuel
                    || instruction.provenance.fuel != leaf.return_fuel
                {
                    return Err(SelectedInstructionError::ProvenancePartitionMismatch {
                        function: function_index,
                    });
                }
            }
            SourceLeafValue::ActiveResidentExactAddOriginalVictimChain(chain) => {
                if block.instructions.len() != 8
                    || block.instructions[0].provenance.fuel != chain.resident.fuel
                    || block.instructions[1].provenance.fuel != chain.left.fuel
                    || block.instructions[2].provenance.fuel != chain.right.fuel
                    || block.instructions[3].provenance.fuel != chain.inner.fuel
                    || block.instructions[4].provenance.fuel != chain.middle.fuel
                    || block.instructions[5].provenance.fuel != chain.bridge.fuel
                    || block.instructions[6].provenance.fuel != chain.join.fuel
                    || block.instructions[7].provenance.fuel != chain.result.fuel
                    || instruction.provenance.fuel != leaf.return_fuel
                {
                    return Err(SelectedInstructionError::ProvenancePartitionMismatch {
                        function: function_index,
                    });
                }
            }
        }
    }
    Ok(())
}

fn terminator_instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    match terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
}
