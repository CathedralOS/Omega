use crate::RelocationPlanningInput;
use crate::data_address_records::insert_data_address_relocations;
use crate::lookups::find_host_binding;
use crate::offsets::FieldModelCallShape;
use crate::offsets::{
    constant_result_data_relocation_offset_for_target,
    data_address_relocation_offset_for_target_with_plan,
};
use omega_calling_conventions::{HostBindingMechanism, HostOperationKey};
use omega_object_file::{
    ObjectSymbolHandle, RelocationPlan, object_symbol_handle_by_name, storage_region_symbol_name,
};
use omega_target_operations::InstructionOperandLike;

fn runtime_operand_storage_region(
    operand: &omega_target_operations::InstructionOperand,
) -> Option<omega_target_operations::RuntimeStorageRegion> {
    operand
        .runtime_string_pointer()
        .map(|(region, _)| region)
        .or_else(|| operand.runtime_string_length().map(|(region, _)| region))
        .or_else(|| {
            operand
                .runtime_pointee_string_pointer()
                .map(|(region, _)| region)
        })
        .or_else(|| {
            operand
                .runtime_pointee_string_length()
                .map(|(region, _)| region)
        })
        .or_else(|| {
            operand
                .runtime_scalar_integer()
                .map(|(region, _, _)| region)
        })
        .or_else(|| operand.runtime_scalar_float().map(|(region, _, _)| region))
        .or_else(|| {
            operand
                .runtime_homogeneous_float_aggregate()
                .map(|(region, _, _, _)| region)
        })
        .or_else(|| {
            operand
                .runtime_system_v_aggregate()
                .map(|(region, _, _, _, _)| region)
        })
        .or_else(|| {
            operand
                .runtime_small_aggregate()
                .map(|(region, _, _, _)| region)
        })
        .or_else(|| {
            operand
                .runtime_large_aggregate()
                .map(|(region, _, _, _)| region)
        })
        .or_else(|| operand.runtime_storage_address().map(|(region, _)| region))
}

pub(super) fn collect_data_address_relocations(
    input: RelocationPlanningInput<'_>,
    function_symbol_handle: ObjectSymbolHandle,
    selected_instruction_index: u32,
    operation_key: Option<HostOperationKey>,
    operands: psi_arena::HandleSpan<omega_target_operations::InstructionOperand>,
    selected_text_offset: usize,
    relocation_plan: &mut RelocationPlan,
) {
    let Some(operands) = input
        .assigned_target_operations
        .instruction_operands(operands)
    else {
        return;
    };

    let selected_binding = operation_key.and_then(|key| find_host_binding(input, key));
    // A Linux syscall host call lays its arguments out differently than a win32
    // import call, so the data-address fixup offset must use the syscall layout.
    let is_syscall = selected_binding
        .is_some_and(|binding| matches!(binding.mechanism, HostBindingMechanism::Syscall { .. }));
    // An AUTHORED import (custom capability + Import mechanism) always rides
    // the value-returning layout on aarch64 -- the operand fixups shift the
    // same way the BL does (see external_calls.rs).
    let authored_import = operation_key.is_some_and(|key| {
        matches!(
            key.capability,
            omega_calling_conventions::HostCapability::Custom(_)
                | omega_calling_conventions::HostCapability::Unknown
        ) && selected_binding
            .is_some_and(|binding| matches!(binding.mechanism, HostBindingMechanism::Import { .. }))
    });
    let authoritative_plan = selected_binding.map(|binding| binding.call_plan());
    // A field-model call's fixup layout depends on the mechanism's shape:
    // whether the receiver is a wire argument (This-call vtable) or
    // dispatch-only (service table), and whether a result place leads the
    // operands (more operands than declared parameters).
    let field_model_shape = selected_binding.and_then(|binding| match &binding.mechanism {
        HostBindingMechanism::VtableSlot { .. } => Some(FieldModelCallShape {
            passes_receiver: true,
            result_present: false,
        }),
        HostBindingMechanism::VtableField { .. } => {
            authoritative_plan.map(|plan| FieldModelCallShape {
                passes_receiver: true,
                result_present: operands.len() > plan.parameters.len(),
            })
        }
        HostBindingMechanism::TableFunction { .. } => {
            authoritative_plan.map(|plan| FieldModelCallShape {
                passes_receiver: false,
                result_present: operands.len() > plan.parameters.len() + 1,
            })
        }
        _ => None,
    });

    for (operand_index, operand) in operands.iter().enumerate() {
        if let Some(data) = operand.data_address() {
            if !data.is_valid() {
                continue;
            }
            let symbol = object_symbol_handle_by_name(
                &input.object,
                input.data.objects.get(data).symbol.as_ref(),
            );
            insert_data_address_relocations(
                input,
                relocation_plan,
                function_symbol_handle,
                selected_instruction_index,
                match selected_binding {
                    Some(binding) => data_address_relocation_offset_for_target_with_plan(
                        input.target,
                        operation_key,
                        operands,
                        selected_text_offset,
                        operand_index,
                        is_syscall,
                        field_model_shape,
                        authored_import,
                        binding.call_plan(),
                    ),
                    None => constant_result_data_relocation_offset_for_target(
                        input.target,
                        selected_text_offset,
                    ),
                },
                symbol,
            );
            continue;
        }

        let region = runtime_operand_storage_region(operand);

        if let Some(region) = region {
            let symbol_name = storage_region_symbol_name(region, input.entry_machine_name);
            let symbol = object_symbol_handle_by_name(&input.object, &symbol_name);
            insert_data_address_relocations(
                input,
                relocation_plan,
                function_symbol_handle,
                selected_instruction_index,
                match selected_binding {
                    Some(binding) => data_address_relocation_offset_for_target_with_plan(
                        input.target,
                        operation_key,
                        operands,
                        selected_text_offset,
                        operand_index,
                        is_syscall,
                        field_model_shape,
                        authored_import,
                        binding.call_plan(),
                    ),
                    None => constant_result_data_relocation_offset_for_target(
                        input.target,
                        selected_text_offset,
                    ),
                },
                symbol,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, SystemVEightbyteClass, ValueShape, evaluate_call_plan,
    };
    use omega_target_operations::{
        InstructionOperand, InstructionOperandKind, RuntimeStorageRegion,
    };

    fn system_v_aggregate(region: RuntimeStorageRegion, byte_offset: usize) -> InstructionOperand {
        InstructionOperand {
            kind: InstructionOperandKind::RuntimeSystemVAggregate {
                region,
                byte_offset,
                byte_count: 16,
                alignment: 8,
                sse_eightbytes: 0b10,
            },
        }
    }

    #[test]
    fn system_v_aggregate_storage_region_and_field_coordinates_are_exact() {
        let result = system_v_aggregate(RuntimeStorageRegion::Machine, 128);
        let receiver = InstructionOperand {
            kind: InstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::Machine,
                byte_offset: 0,
                byte_count: 8,
            },
        };
        let argument = system_v_aggregate(RuntimeStorageRegion::Machine, 112);
        let immediate = InstructionOperand {
            kind: InstructionOperandKind::ImmediateInteger(7),
        };
        assert_eq!(
            [
                runtime_operand_storage_region(&result),
                runtime_operand_storage_region(&receiver),
                runtime_operand_storage_region(&argument),
                runtime_operand_storage_region(&immediate),
            ],
            [
                Some(RuntimeStorageRegion::Machine),
                Some(RuntimeStorageRegion::Machine),
                Some(RuntimeStorageRegion::Machine),
                None,
            ],
        );

        let aggregate_shape = ValueShape::system_v_aggregate(
            16,
            8,
            SystemVEightbyteClass::Integer,
            SystemVEightbyteClass::Sse,
        );
        let plan = evaluate_call_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8), aggregate_shape],
                result: Some(aggregate_shape),
            },
        )
        .expect("exact SysV mixed-aggregate plan");
        let operands = [result, receiver, argument];
        let shape = Some(FieldModelCallShape {
            passes_receiver: true,
            result_present: true,
        });
        let instruction_text_offset = 446;
        let selected_text_offset = instruction_text_offset
            + omega_instruction_selection::foreign_float_control_prefix_width(
                omega_target::Architecture::X86_64,
            );
        let coordinates = [0, 1, 2].map(|operand_index| {
            data_address_relocation_offset_for_target_with_plan(
                omega_target::NativeTarget::linux_x64(),
                Some(HostOperationKey::default()),
                &operands,
                selected_text_offset,
                operand_index,
                false,
                shape,
                false,
                &plan,
            )
        });

        assert_eq!(coordinates, [516, 460, 477]);
    }
}
