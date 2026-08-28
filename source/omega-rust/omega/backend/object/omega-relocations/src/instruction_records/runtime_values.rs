use super::context::InstructionRelocationContext;
use omega_assigned_target_operations::RuntimeValueOperand;
use omega_target_operations::RuntimeValueOperandHandle;

pub(super) fn collect_runtime_value_operand_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    operand_text_offset: usize,
    operand: RuntimeValueOperandHandle,
) {
    let Some(operand) = context
        .input
        .assigned_target_operations
        .runtime_value_operand(operand)
    else {
        return;
    };

    match &operand.kind {
        RuntimeValueOperand::Immediate(_) => {}
        RuntimeValueOperand::Storage { region, .. } => {
            let symbol = context.storage_region_symbol_handle(*region);
            context.insert_data_address(operand_text_offset, symbol);
        }
        RuntimeValueOperand::BitField { region, .. } => {
            let symbol = context.storage_region_symbol_handle(*region);
            context.insert_data_address(operand_text_offset, symbol);
        }
        RuntimeValueOperand::Pointee { .. }
        | RuntimeValueOperand::FrameBaseIndexed { .. }
        | RuntimeValueOperand::FrameFixedIndexed { .. } => {
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address(operand_text_offset, symbol);
        }
        RuntimeValueOperand::FrameIndexed { index_region, .. } => {
            let frame_symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address(operand_text_offset, frame_symbol);
            if *index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                let machine_symbol = context.storage_region_symbol_handle(
                    omega_target_operations::RuntimeStorageRegion::Machine,
                );
                context.insert_data_address(
                    operand_text_offset
                        + omega_instruction_selection::frame_indexed_operand_machine_index_base_offset(
                            context.input.target.architecture,
                        ),
                    machine_symbol,
                );
            }
        }
        RuntimeValueOperand::MachineIndexed { index_region, .. } => {
            // MACHINE base at the operand start; a FRAME-resident index
            // materializes the frame base as a SECOND pair at the
            // architecture's pinned offset (both encoders emit the pairs
            // unconditionally in that order so these offsets are constant).
            let machine_symbol = context.storage_region_symbol_handle(
                omega_target_operations::RuntimeStorageRegion::Machine,
            );
            context.insert_data_address(operand_text_offset, machine_symbol);
            if *index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                let frame_symbol = context.runtime_frame_symbol_handle();
                context.insert_data_address(
                    operand_text_offset
                        + omega_instruction_selection::machine_indexed_operand_frame_index_base_offset(
                            context.input.target.architecture,
                        ),
                    frame_symbol,
                );
            }
        }
        RuntimeValueOperand::Binary { left, right, .. } => {
            collect_runtime_value_operand_relocations(context, operand_text_offset, *left);
            let left_width = omega_instruction_selection::runtime_value_operand_width(
                context.input.target.architecture,
                context.input.assigned_target_operations,
                *left,
            );
            let right_offset = operand_text_offset
                + left_width
                + omega_instruction_selection::runtime_binary_right_operand_gap(
                    context.input.target.architecture,
                );
            collect_runtime_value_operand_relocations(context, right_offset, *right);
        }
        RuntimeValueOperand::TextEquals {
            left_region,
            right_region,
            ..
        } => {
            // Two descriptor-base relocations: the left base sits at the
            // operand start, the right base at the architecture's pinned
            // right-base offset (the encoders keep the prefix fixed-width).
            let left_symbol = context.storage_region_symbol_handle(*left_region);
            context.insert_data_address(operand_text_offset, left_symbol);
            let right_symbol = context.storage_region_symbol_handle(*right_region);
            let right_offset = operand_text_offset
                + omega_instruction_selection::runtime_text_equals_right_base_offset(
                    context.input.target.architecture,
                );
            context.insert_data_address(right_offset, right_symbol);
        }
        RuntimeValueOperand::TextEqualsLiteral { place, .. } => {
            // The place address setup leads the encoding; recurse through that
            // canonical operand so indexed places retain every secondary base
            // relocation as well. The literal bytes are inline immediates.
            collect_runtime_value_operand_relocations(context, operand_text_offset, *place);
        }
        RuntimeValueOperand::Convert { source, .. } => {
            // The source is loaded first (at the operand's own text offset); the
            // in-place convert that follows carries no relocation of its own.
            collect_runtime_value_operand_relocations(context, operand_text_offset, *source);
        }
    }
}
