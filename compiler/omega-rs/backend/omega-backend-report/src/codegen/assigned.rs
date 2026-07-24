use super::runtime_values::runtime_value_operand_name;
use crate::BackendReportInput;
use omega_assigned_target_operations::{
    AssignedRegisterName, AssignedValueHomeKind, AssignedValueOperand, RuntimeValueOperand,
    X86_64AssignedRegister,
};

pub(super) fn write_assigned_target_operations_section(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
) {
    output.push_str("## Assigned Target Operations\n");
    output.push_str(&format!(
        "functions: {}\n",
        backend_plan.assigned_target_operations.code.functions.len()
    ));
    output.push_str(&format!(
        "instructions: {}\n",
        backend_plan
            .assigned_target_operations
            .code
            .instructions
            .len()
    ));
    output.push_str(&format!(
        "runtime value homes: {}\n",
        backend_plan
            .assigned_target_operations
            .runtime_values_with_homes()
            .count()
    ));
    let scratch_home_count = backend_plan.assigned_target_operations.scratch_home_count();
    output.push_str(&format!("scratch homes: {}\n", scratch_home_count));
    write_assigned_value_homes(output, backend_plan);
    output.push('\n');
}

fn write_assigned_value_homes(output: &mut String, backend_plan: &BackendReportInput<'_>) {
    if backend_plan
        .assigned_target_operations
        .runtime_values_with_homes()
        .next()
        .is_none()
    {
        output.push_str("homes: none\n");
        return;
    }

    output.push_str("homes:\n");
    for (operand_handle, operand) in backend_plan
        .assigned_target_operations
        .runtime_values_with_homes()
    {
        output.push_str(&format!(
            "  - #{} {} => {}\n",
            operand_handle.arena_index(),
            runtime_value_operand_name(backend_plan, operand_handle),
            assigned_value_home_name(operand)
        ));
    }
}

fn assigned_value_home_name(operand: &AssignedValueOperand) -> String {
    match operand.home {
        AssignedValueHomeKind::Immediate => "immediate".to_owned(),
        AssignedValueHomeKind::StackSlot {
            byte_offset,
            byte_size,
        } => format!("stack slot frame@{byte_offset}/{}", byte_size),
        AssignedValueHomeKind::RuntimeStorage {
            region,
            byte_offset,
            byte_size,
        } => format!("storage {region:?}@{byte_offset}/{}", byte_size),
        AssignedValueHomeKind::RuntimePointee {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
        } => format!(
            "pointee frame@{pointer_byte_offset}+{field_byte_offset}/{}",
            byte_size
        ),
        AssignedValueHomeKind::RuntimeFrameIndexed {
            descriptor_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => format!(
            "frame-indexed desc@{descriptor_offset} idx {index_region:?}@{index_offset}/{index_byte_size} elem {element_byte_size} field +{field_byte_offset}/{}",
            byte_size
        ),
        AssignedValueHomeKind::RuntimeFrameBaseIndexed {
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => format!(
            "frame-base-indexed base@{base_byte_offset} idx@{index_offset}/{index_byte_size} elem {element_byte_size} field +{field_byte_offset}/{}",
            byte_size
        ),
        AssignedValueHomeKind::RuntimeFrameFixedIndexed {
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => format!(
            "frame-fixed-indexed desc@{descriptor_offset} idx {element_index} elem {element_byte_size} field +{field_byte_offset}/{}",
            byte_size
        ),
        AssignedValueHomeKind::ScratchRegister { bank, name } => {
            let source = match operand.kind {
                RuntimeValueOperand::Binary { .. } => "binary temp",
                _ => "temp",
            };
            format!(
                "{bank:?} register {} ({source})",
                assigned_register_name(name)
            )
        }
    }
}

fn assigned_register_name(name: AssignedRegisterName) -> String {
    match name {
        AssignedRegisterName::Aarch64X(register) => format!("x{register}"),
        AssignedRegisterName::X86_64(register) => match register {
            X86_64AssignedRegister::R10 => "r10".to_owned(),
            X86_64AssignedRegister::R11 => "r11".to_owned(),
            X86_64AssignedRegister::R12 => "r12".to_owned(),
            X86_64AssignedRegister::R13 => "r13".to_owned(),
            X86_64AssignedRegister::R14 => "r14".to_owned(),
            X86_64AssignedRegister::R15 => "r15".to_owned(),
        },
    }
}
