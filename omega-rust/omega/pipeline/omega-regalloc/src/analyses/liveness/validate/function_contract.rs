//! Exact comparison of proposed and independently replayed function liveness.

use super::shared::*;

pub(super) fn validate_function(
    function_index: usize,
    actual: &FunctionLiveness,
    expected: &FunctionLiveness,
) -> Result<(), LivenessError> {
    if actual.machine != expected.machine || actual.blocks.len() != expected.blocks.len() {
        return Err(LivenessError::FunctionMismatch {
            function: function_index,
        });
    }
    if actual.entry_definitions != expected.entry_definitions
        || actual.operand_positions != expected.operand_positions
    {
        return Err(LivenessError::FixedConstraintMismatch {
            function: function_index,
        });
    }
    let positions = actual
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .map(|instruction| instruction.position.0)
        .collect::<Vec<_>>();
    let expected_position_count =
        u32::try_from(positions.len()).map_err(|_| LivenessError::NonDensePositions {
            function: function_index,
        })?;
    if positions != (0..expected_position_count).collect::<Vec<_>>() {
        return Err(LivenessError::NonDensePositions {
            function: function_index,
        });
    }
    for (actual, expected) in actual.blocks.iter().zip(&expected.blocks) {
        if actual.block != expected.block
            || actual.source_block != expected.source_block
            || actual.instructions.len() != expected.instructions.len()
        {
            return Err(LivenessError::BlockMismatch {
                function: function_index,
                block: expected.block.0,
            });
        }
        for set in [&actual.virtual_live_in, &actual.virtual_live_out] {
            require_canonical(function_index, None, set)?;
        }
        for set in [&actual.unit_live_in, &actual.unit_live_out] {
            require_canonical(function_index, None, set)?;
        }
        if actual.virtual_live_in != expected.virtual_live_in
            || actual.virtual_live_out != expected.virtual_live_out
            || actual.unit_live_in != expected.unit_live_in
            || actual.unit_live_out != expected.unit_live_out
        {
            return Err(LivenessError::BlockMismatch {
                function: function_index,
                block: expected.block.0,
            });
        }
        for (actual_instruction, expected_instruction) in
            actual.instructions.iter().zip(&expected.instructions)
        {
            for set in [
                &actual_instruction.virtual_uses,
                &actual_instruction.virtual_defs,
                &actual_instruction.virtual_live_in,
                &actual_instruction.virtual_live_out,
            ] {
                require_canonical(
                    function_index,
                    Some(expected_instruction.instruction.0),
                    set,
                )?;
            }
            for set in [
                &actual_instruction.unit_uses,
                &actual_instruction.unit_defs,
                &actual_instruction.unit_clobbers,
                &actual_instruction.unit_live_in,
                &actual_instruction.unit_live_out,
            ] {
                require_canonical(
                    function_index,
                    Some(expected_instruction.instruction.0),
                    set,
                )?;
            }
            if actual_instruction.position != expected_instruction.position
                || actual_instruction.instruction != expected_instruction.instruction
                || actual_instruction.virtual_uses != expected_instruction.virtual_uses
                || actual_instruction.virtual_defs != expected_instruction.virtual_defs
                || actual_instruction.unit_uses != expected_instruction.unit_uses
                || actual_instruction.unit_defs != expected_instruction.unit_defs
                || actual_instruction.unit_clobbers != expected_instruction.unit_clobbers
            {
                return Err(LivenessError::InstructionMismatch {
                    function: function_index,
                    instruction: expected_instruction.instruction.0,
                });
            }
            if actual_instruction.virtual_live_in != expected_instruction.virtual_live_in
                || actual_instruction.virtual_live_out != expected_instruction.virtual_live_out
                || actual_instruction.unit_live_in != expected_instruction.unit_live_in
                || actual_instruction.unit_live_out != expected_instruction.unit_live_out
            {
                return Err(LivenessError::TransferMismatch {
                    function: function_index,
                    instruction: expected_instruction.instruction.0,
                });
            }
        }
        if actual.successors.len() != expected.successors.len() {
            return Err(LivenessError::SuccessorMismatch {
                function: function_index,
                block: expected.block.0,
                ordinal: 0,
            });
        }
        for (actual_successor, expected_successor) in
            actual.successors.iter().zip(&expected.successors)
        {
            require_canonical(
                function_index,
                Some(expected_successor.terminator.0),
                &actual_successor.virtual_live,
            )?;
            require_canonical(
                function_index,
                Some(expected_successor.terminator.0),
                &actual_successor.unit_live,
            )?;
            if actual_successor != expected_successor {
                return Err(LivenessError::SuccessorMismatch {
                    function: function_index,
                    block: expected.block.0,
                    ordinal: expected_successor.polarity_ordinal,
                });
            }
        }
    }
    Ok(())
}
