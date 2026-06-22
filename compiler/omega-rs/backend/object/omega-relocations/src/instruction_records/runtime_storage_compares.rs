use super::super::offsets::runtime_storage_compare_right_address_offset;
use super::context::InstructionRelocationContext;
use super::runtime_values::collect_runtime_value_operand_relocations;
use omega_instruction_selection::dispatch_guard_compare_static_width;
use omega_target_operations::{SelectedInstructionKind, StateGuardLowering, StateGuardOperator};

pub(super) fn collect_runtime_storage_compare_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    match instruction {
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::CompareStaticValue,
            operator:
                StateGuardOperator::Equal
                | StateGuardOperator::NotEqual
                | StateGuardOperator::Greater
                | StateGuardOperator::GreaterOrEqual
                | StateGuardOperator::Less
                | StateGuardOperator::LessOrEqual
                | StateGuardOperator::GreaterUnsigned
                | StateGuardOperator::GreaterOrEqualUnsigned
                | StateGuardOperator::LessUnsigned
                | StateGuardOperator::LessOrEqualUnsigned,
            storage_region,
            byte_offset,
            byte_size,
            has_storage: true,
            is_float,
            ..
        } => {
            // The guard's Absolute64 storage-address relocation only exists when the guard
            // actually emits a storage load with an inline 64-bit address immediate. On targets
            // where `EvaluateDispatchGuard` lowers to a zero-byte instruction (e.g. x86_64, where
            // the comparison is folded into the following `DispatchCaseEnter`'s `cmp r12d, N`),
            // there is no immediate to relocate. The guard's text offset then coincides with the
            // *next* instruction (a `SetDispatchState` / `mov r12d, imm32`), so emitting a
            // relocation here would splatter the 8-byte storage address across that instruction's
            // 4-byte index immediate and corrupt the dispatch index — the `0xC0000005` crash.
            // Only anchor the relocation when the guard occupies real bytes.
            if dispatch_guard_compare_static_width(
                context.input.target.architecture,
                *byte_offset,
                *byte_size,
                *is_float,
            ) != 0
            {
                let symbol = context.storage_region_symbol_handle(*storage_region);
                context.insert_data_address_at_instruction_start(symbol);
            }
            true
        }
        SelectedInstructionKind::CompareRuntimeStorage {
            left_region,
            right_region,
            byte_size,
            ..
        } => {
            let left_symbol = context.storage_region_symbol_handle(*left_region);
            let right_symbol = context.storage_region_symbol_handle(*right_region);
            context.insert_data_address_at_instruction_start(left_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_storage_compare_right_address_offset(
                    context.input.target.architecture,
                    *byte_size,
                ),
                right_symbol,
            );
            true
        }
        SelectedInstructionKind::CompareRuntimeStorageValue { region, .. } => {
            let symbol = context.storage_region_symbol_handle(*region);
            context.insert_data_address_at_instruction_start(symbol);
            true
        }
        SelectedInstructionKind::CompareRuntimeValues { left, right, .. } => {
            let base_offset = context.selected_text_offset;
            collect_runtime_value_operand_relocations(context, base_offset, *left);
            let left_width = omega_instruction_selection::runtime_value_operand_width(
                context.input.target.architecture,
                context.input.assigned_target_operations,
                *left,
            );
            collect_runtime_value_operand_relocations(context, base_offset + left_width, *right);
            true
        }
        _ => false,
    }
}
