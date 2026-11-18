use machine_code::layout_requirements::*;
use post_allocation_machine::PostAllocationMachineProgram;
use selected_instructions::{SelectedInstructionKind as Kind, SelectedTerminator as Term};

pub fn prepare_layout_requirements(
    program: &PostAllocationMachineProgram,
) -> EncodedLayoutRequirements {
    EncodedLayoutRequirements {
        selected: program.machine.selected,
        machine: program.machine.identity,
        target: program.target,
        functions: program
            .functions
            .iter()
            .map(prepare_function_control)
            .collect(),
        alternatives: program
            .machine
            .functions
            .iter()
            .map(prepare_function_alternatives)
            .collect(),
    }
}
pub fn prepare_function_control(
    function: &selected_instructions::SelectedFunction,
) -> EncodedLayoutFunction {
    EncodedLayoutFunction {
        machine: function.machine,
        entry_block: function.entry_block,
        blocks: function.blocks.iter().map(prepare_block_control).collect(),
    }
}
pub fn prepare_block_control(block: &selected_instructions::SelectedBlock) -> EncodedLayoutBlock {
    let successor = |s: &selected_instructions::SelectedSuccessor| EncodedLayoutSuccessor {
        psi_edge: s.psi_edge,
        block: s.block,
    };
    let instruction = prepare_row_control(selected_instructions::terminator_instruction(
        &block.terminator,
    ));
    let terminator = match &block.terminator {
        Term::Return { .. } => EncodedLayoutTerminator::Return { instruction },
        Term::Jump { successor: s, .. } => EncodedLayoutTerminator::Jump {
            instruction,
            successor: successor(s),
        },
        Term::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } => EncodedLayoutTerminator::ConditionalBranch {
            instruction,
            when_nonzero: successor(when_nonzero),
            when_zero: successor(when_zero),
        },
        Term::ConditionalBranchU64LessThan {
            when_less,
            when_not_less,
            ..
        } => EncodedLayoutTerminator::ConditionalBranchU64LessThan {
            instruction,
            when_less: successor(when_less),
            when_not_less: successor(when_not_less),
        },
        Term::ConditionalBranchI64LessThan {
            when_less,
            when_not_less,
            ..
        } => EncodedLayoutTerminator::ConditionalBranchI64LessThan {
            instruction,
            when_less: successor(when_less),
            when_not_less: successor(when_not_less),
        },
        _ => EncodedLayoutTerminator::Stop { instruction },
    };
    EncodedLayoutBlock {
        id: block.id,
        instructions: block.instructions.iter().map(prepare_row_control).collect(),
        terminator,
    }
}
pub fn prepare_row_control(row: &selected_instructions::SelectedInstruction) -> EncodedLayoutRow {
    use EncodedLayoutOperation as Op;
    let kind = match row.kind {
        Kind::CompareI64Zero => Op::CompareI64Zero,
        Kind::CopyI64 => Op::CopyI64,
        Kind::ConditionalBranchNonZero => Op::ConditionalBranchNonZero,
        Kind::ConditionalBranchU64LessThan => Op::ConditionalBranchU64LessThan,
        Kind::ConditionalBranchI64LessThan => Op::ConditionalBranchI64LessThan,
        selected_instructions::selected_call_pattern!(..) => Op::Call {
            callee: row.kind.machine_callee(),
        },
        Kind::NormalizedForeignCall { boundary, ordinal } => {
            Op::NormalizedForeignCall { boundary, ordinal }
        }
        Kind::ImageAddress { reference } => Op::ImageAddress { reference },
        _ => Op::Ordinary,
    };
    EncodedLayoutRow { id: row.id, kind }
}
pub fn prepare_function_alternatives(
    function: &post_allocation_machine::PostAllocationMachineFunction,
) -> EncodedFunctionAlternatives {
    EncodedFunctionAlternatives {
        machine: function.machine,
        blocks: function
            .blocks
            .iter()
            .map(|block| EncodedBlockAlternatives {
                block: block.block,
                instructions: block.instructions.iter().map(prepare_alternative).collect(),
            })
            .collect(),
    }
}
pub fn prepare_alternative(
    row: &post_allocation_machine::PostAllocationMachineInstruction,
) -> EncodedAlternativeRequirement {
    EncodedAlternativeRequirement {
        instruction: row.instruction,
        alternative: EncodedAlternativeDeclaration {
            key: row.alternative.key,
            size: row.alternative.size,
            encoded: row.alternative.encoded.clone(),
        },
    }
}
