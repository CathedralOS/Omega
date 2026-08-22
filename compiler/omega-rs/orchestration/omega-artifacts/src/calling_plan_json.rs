//! Canonical JSON projection for calling plans and value placements.

use omega_calling_conventions::{
    BoundaryEntryPlan, CallingPolicy, EntryControl, EntryStack, IndirectPointerLocation,
    MachineRegime, MachineRegister, Preemption, RegisterSet, SystemVEightbyteClass, ValueClass,
    ValueLocation, ValuePlacement, ValueShape,
};

use super::push_hex_u16;

pub(super) fn push_boundary_plan_json(output: &mut String, plan: &BoundaryEntryPlan) {
    output.push_str("{\"call\": {\"policy\": \"");
    output.push_str(calling_policy_name(plan.call.policy));
    output.push_str("\", \"parameters\": [");
    for (index, placement) in plan.call.parameters.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        push_value_placement_json(output, placement);
    }
    output.push_str("], \"result\": ");
    if let Some(result) = &plan.call.result {
        push_value_placement_json(output, result);
    } else {
        output.push_str("null");
    }
    output.push_str(", \"ordinary_clobbers\": ");
    push_register_set_json(output, &plan.call.ordinary_clobbers);
    output.push_str(", \"stack_alignment\": ");
    output.push_str(&plan.call.stack_alignment.to_string());
    output.push_str(", \"shadow_bytes\": ");
    output.push_str(&plan.call.shadow_bytes.to_string());
    output.push_str(", \"entry_control\": ");
    push_entry_control_json(output, plan.call.entry_control);
    output.push_str("}, \"state\": {\"initial_regime\": ");
    push_machine_regime_json(output, plan.state.initial_regime);
    output.push_str(", \"interrupted_state_bits\": ");
    push_hex_u16(output, plan.state.interrupted_state.bits());
    output.push_str(", \"saved_state_bits\": ");
    push_hex_u16(output, plan.state.saved_state.bits());
    output.push_str(", \"restored_state_bits\": ");
    push_hex_u16(output, plan.state.restored_state.bits());
    output.push_str(", \"permitted_transitive_use_bits\": ");
    push_hex_u16(output, plan.state.permitted_transitive_use.bits());
    output.push_str(", \"stack\": ");
    push_entry_stack_json(output, plan.state.stack);
    output.push_str(", \"preemption\": ");
    push_preemption_json(output, plan.state.preemption);
    output.push_str("}}");
}

fn push_value_placement_json(output: &mut String, placement: &ValuePlacement) {
    output.push_str("{\"shape\": ");
    push_value_shape_json(output, placement.shape);
    output.push_str(", \"locations\": [");
    for (index, location) in placement.locations.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        push_value_location_json(output, *location);
    }
    output.push_str("]}");
}

/// Machine-readable form of one already-normalized calling-plan placement.
/// Consumers may embed this object in a larger artifact without rebuilding
/// target register, stack, indirect-copy, or aggregate-shape vocabulary.
pub fn value_placement_json(placement: &ValuePlacement) -> String {
    let mut output = String::new();
    push_value_placement_json(&mut output, placement);
    output
}

fn push_value_shape_json(output: &mut String, shape: ValueShape) {
    output.push_str("{\"class\": ");
    match shape.class {
        ValueClass::Integer => output.push_str("\"integer\""),
        ValueClass::Float => output.push_str("\"float\""),
        ValueClass::HomogeneousFloatAggregate { members } => {
            output.push_str("{\"homogeneous_float_aggregate\": ");
            output.push_str(&members.to_string());
            output.push('}');
        }
        ValueClass::SystemVAggregate { first, second } => {
            output.push_str("{\"system_v_aggregate\": [\"");
            output.push_str(system_v_class_name(first));
            output.push_str("\", \"");
            output.push_str(system_v_class_name(second));
            output.push_str("\"]}");
        }
    }
    output.push_str(", \"byte_size\": ");
    output.push_str(&shape.byte_size.to_string());
    output.push_str(", \"alignment\": ");
    output.push_str(&shape.alignment.to_string());
    output.push('}');
}

fn push_value_location_json(output: &mut String, location: ValueLocation) {
    match location {
        ValueLocation::Register {
            register,
            value_byte_offset,
            byte_size,
        } => {
            output.push_str("{\"register\": ");
            push_register_json(output, register);
            output.push_str(", \"value_byte_offset\": ");
            output.push_str(&value_byte_offset.to_string());
            output.push_str(", \"byte_size\": ");
            output.push_str(&byte_size.to_string());
            output.push('}');
        }
        ValueLocation::Stack {
            stack_byte_offset,
            value_byte_offset,
            byte_size,
            alignment,
        } => {
            output.push_str("{\"stack_byte_offset\": ");
            output.push_str(&stack_byte_offset.to_string());
            output.push_str(", \"value_byte_offset\": ");
            output.push_str(&value_byte_offset.to_string());
            output.push_str(", \"byte_size\": ");
            output.push_str(&byte_size.to_string());
            output.push_str(", \"alignment\": ");
            output.push_str(&alignment.to_string());
            output.push('}');
        }
        ValueLocation::Indirect {
            pointer,
            copy_stack_byte_offset,
            byte_size,
            alignment,
        } => {
            output.push_str("{\"indirect\": {\"pointer\": ");
            push_indirect_pointer_json(output, pointer);
            output.push_str(", \"copy_stack_byte_offset\": ");
            if let Some(offset) = copy_stack_byte_offset {
                output.push_str(&offset.to_string());
            } else {
                output.push_str("null");
            }
            output.push_str(", \"byte_size\": ");
            output.push_str(&byte_size.to_string());
            output.push_str(", \"alignment\": ");
            output.push_str(&alignment.to_string());
            output.push_str("}}");
        }
    }
}

fn push_indirect_pointer_json(output: &mut String, pointer: IndirectPointerLocation) {
    match pointer {
        IndirectPointerLocation::Register(register) => {
            output.push_str("{\"register\": ");
            push_register_json(output, register);
            output.push('}');
        }
        IndirectPointerLocation::Stack {
            stack_byte_offset,
            alignment,
        } => {
            output.push_str("{\"stack_byte_offset\": ");
            output.push_str(&stack_byte_offset.to_string());
            output.push_str(", \"alignment\": ");
            output.push_str(&alignment.to_string());
            output.push('}');
        }
    }
}

pub(super) fn push_register_set_json(output: &mut String, registers: &RegisterSet) {
    output.push('[');
    for (index, register) in registers.as_slice().iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        push_register_json(output, *register);
    }
    output.push(']');
}

fn push_register_json(output: &mut String, register: MachineRegister) {
    output.push('"');
    match register {
        MachineRegister::X86Rax => output.push_str("x86_rax"),
        MachineRegister::X86Rcx => output.push_str("x86_rcx"),
        MachineRegister::X86Rdx => output.push_str("x86_rdx"),
        MachineRegister::X86Rbx => output.push_str("x86_rbx"),
        MachineRegister::X86Rsp => output.push_str("x86_rsp"),
        MachineRegister::X86Rbp => output.push_str("x86_rbp"),
        MachineRegister::X86Rsi => output.push_str("x86_rsi"),
        MachineRegister::X86Rdi => output.push_str("x86_rdi"),
        MachineRegister::X86R8 => output.push_str("x86_r8"),
        MachineRegister::X86R9 => output.push_str("x86_r9"),
        MachineRegister::X86R10 => output.push_str("x86_r10"),
        MachineRegister::X86R11 => output.push_str("x86_r11"),
        MachineRegister::X86R12 => output.push_str("x86_r12"),
        MachineRegister::X86R13 => output.push_str("x86_r13"),
        MachineRegister::X86R14 => output.push_str("x86_r14"),
        MachineRegister::X86R15 => output.push_str("x86_r15"),
        MachineRegister::X86Xmm(index) => output.push_str(&format!("x86_xmm{index}")),
        MachineRegister::Aarch64X(index) => output.push_str(&format!("aarch64_x{index}")),
        MachineRegister::Aarch64V(index) => output.push_str(&format!("aarch64_v{index}")),
    }
    output.push('"');
}

fn push_entry_control_json(output: &mut String, control: EntryControl) {
    match control {
        EntryControl::CallReturn => output.push_str("\"call_return\""),
        EntryControl::InterruptReturn => output.push_str("\"interrupt_return\""),
        EntryControl::SupervisorCall {
            number_register,
            immediate,
        } => {
            output.push_str("{\"supervisor_call\": {\"number_register\": ");
            push_register_json(output, number_register);
            output.push_str(", \"immediate\": ");
            output.push_str(&immediate.to_string());
            output.push_str("}}");
        }
    }
}

fn push_machine_regime_json(output: &mut String, regime: MachineRegime) {
    match regime {
        MachineRegime::X86Long64 => output.push_str("\"x86_long64\""),
        MachineRegime::Aarch64A64 { exception_level } => {
            output.push_str("{\"aarch64_a64\": {\"exception_level\": ");
            output.push_str(&exception_level.to_string());
            output.push_str("}}");
        }
    }
}

pub(super) fn push_entry_stack_json(output: &mut String, stack: EntryStack) {
    match stack {
        EntryStack::Interrupted => output.push_str("\"interrupted\""),
        EntryStack::Dedicated { class } => {
            output.push_str("{\"dedicated\": {\"class\": ");
            output.push_str(&class.to_string());
            output.push_str("}}");
        }
        EntryStack::ProviderSelected => output.push_str("\"provider_selected\""),
    }
}

fn push_preemption_json(output: &mut String, preemption: Preemption) {
    match preemption {
        Preemption::NotApplicable => output.push_str("\"not_applicable\""),
        Preemption::Masked => output.push_str("\"masked\""),
        Preemption::Nestable { maximum_depth } => {
            output.push_str("{\"nestable\": {\"maximum_depth\": ");
            output.push_str(&maximum_depth.to_string());
            output.push_str("}}");
        }
        Preemption::ProviderDefined => output.push_str("\"provider_defined\""),
    }
}

const fn calling_policy_name(policy: CallingPolicy) -> &'static str {
    match policy {
        CallingPolicy::MicrosoftX64 => "microsoft_x64",
        CallingPolicy::SystemVAMD64 => "system_v_amd64",
        CallingPolicy::Aapcs64 => "aapcs64",
        CallingPolicy::LinuxSyscallX86_64 => "linux_syscall_x86_64",
        CallingPolicy::LinuxSyscallAarch64 => "linux_syscall_aarch64",
    }
}

const fn system_v_class_name(class: SystemVEightbyteClass) -> &'static str {
    match class {
        SystemVEightbyteClass::Integer => "integer",
        SystemVEightbyteClass::Sse => "sse",
    }
}
