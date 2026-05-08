use crate::host_calls::HostCallArgumentKind;
use crate::identity::expressions::count_expression_strings;
use crate::identity::NativeStringStorage;
use crate::plan::NativePlan;

pub(in crate::identity) fn count_host_call_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, call) in native_plan.host_calls.calls.iter() {
        storage.count_program_name_identity(&call.machine);
        storage.count_program_name_identity(&call.state);
        storage.count_identity(&call.platform_call);
    }
    for (_, unsupported) in native_plan.host_calls.unsupported_calls.iter() {
        storage.count_program_name_identity(&unsupported.machine);
        storage.count_program_name_identity(&unsupported.state);
        storage.count_identity(&unsupported.platform_call);
        storage.count_report(&unsupported.reason);
    }
    for (_, operation) in native_plan.host_calls.operations.iter() {
        storage.count_identity(&operation.capability);
        storage.count_identity(&operation.operation);
    }
    for (_, argument) in native_plan.host_calls.arguments.iter() {
        match &argument.kind {
            HostCallArgumentKind::Text(value) => storage.count_payload(value),
            HostCallArgumentKind::Expression(expression) => {
                count_expression_strings(expression, storage);
            }
            HostCallArgumentKind::Integer(_) => {}
        }
    }
}

pub(in crate::identity) fn count_state_call_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, call) in native_plan.state_calls.calls.iter() {
        storage.count_program_name_identity(&call.source_machine);
        storage.count_program_name_identity(&call.source_state);
        storage.count_program_name_identity(&call.receiver);
        storage.count_program_name_identity(&call.target_machine);
        storage.count_program_name_identity(&call.target_state);
    }
    for (_, argument) in native_plan.state_calls.arguments.iter() {
        storage.count_program_name_identity(&argument.parameter_name);
        count_expression_strings(&argument.expression, storage);
    }
}

pub(in crate::identity) fn count_alias_flow_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, alias) in native_plan.alias_flow.aliases.iter() {
        storage.count_program_name_identity(&alias.caller_machine);
        storage.count_program_name_identity(&alias.caller_state);
        storage.count_program_name_identity(&alias.callee_machine);
        storage.count_program_name_identity(&alias.callee_state);
        storage.count_program_name_identity(&alias.parameter_name);
        count_expression_strings(&alias.argument, storage);
    }
}

pub(in crate::identity) fn count_state_storage_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, local) in native_plan.state_storage.locals.iter() {
        storage.count_program_name_identity(&local.machine);
        storage.count_program_name_identity(&local.state);
        storage.count_program_name_identity(&local.name);
        storage.count_identity(&local.type_name);
    }
    for (_, mutation) in native_plan.state_storage.mutations.iter() {
        storage.count_program_name_identity(&mutation.machine);
        storage.count_program_name_identity(&mutation.state);
        count_expression_strings(&mutation.target, storage);
        count_expression_strings(&mutation.value, storage);
    }
}

pub(in crate::identity) fn count_state_value_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, value) in native_plan.state_values.values.iter() {
        storage.count_program_name_identity(&value.machine);
        storage.count_program_name_identity(&value.state);
        count_expression_strings(&value.expression, storage);
    }
}

pub(in crate::identity) fn count_runtime_storage_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, slot) in native_plan.runtime_storage.frame_slots.iter() {
        storage.count_program_name_identity(&slot.source_machine);
        storage.count_program_name_identity(&slot.source_state);
        storage.count_program_name_identity(&slot.name);
        storage.count_identity(&slot.type_name);
    }
    for (_, write) in native_plan.runtime_storage.writes.iter() {
        storage.count_program_name_identity(&write.source_machine);
        storage.count_program_name_identity(&write.source_state);
        count_expression_strings(&write.target, storage);
        count_expression_strings(&write.value, storage);
    }
}

pub(in crate::identity) fn count_runtime_text_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, text_use) in native_plan.runtime_text.uses.iter() {
        storage.count_program_name_identity(&text_use.machine);
        storage.count_program_name_identity(&text_use.state);
        storage.count_identity(&text_use.platform_call);
        count_expression_strings(&text_use.expression, storage);
    }
    for (_, buffer) in native_plan.runtime_text.buffers.iter() {
        storage.count_program_name_identity(&buffer.machine);
        storage.count_program_name_identity(&buffer.state);
        storage.count_identity(&buffer.platform_call);
        count_expression_strings(&buffer.target, storage);
    }
    for (_, slot) in native_plan.runtime_text.slots.iter() {
        count_expression_strings(&slot.place, storage);
    }
    for (_, write) in native_plan.runtime_text.writes.iter() {
        storage.count_program_name_identity(&write.machine);
        storage.count_program_name_identity(&write.state);
        count_expression_strings(&write.target, storage);
        count_expression_strings(&write.value, storage);
    }
    for (_, builder) in native_plan.runtime_text.builders.iter() {
        storage.count_program_name_identity(&builder.machine);
        storage.count_program_name_identity(&builder.state);
        count_expression_strings(&builder.target, storage);
    }
    for (_, segment) in native_plan.runtime_text.builder_segments.iter() {
        count_expression_strings(&segment.expression, storage);
    }
}

pub(in crate::identity) fn count_layout_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, data_layout) in native_plan.layouts.data_layouts.iter() {
        storage.count_program_name_identity(&data_layout.name);
        if let crate::layout::DataShape::Enum { variants } = &data_layout.shape {
            for variant in variants {
                storage.count_program_name_identity(variant);
            }
        }
    }
    for (_, field) in native_plan.layouts.fields.iter() {
        storage.count_program_name_identity(&field.name);
        storage.count_identity(&field.type_name);
    }
    for (_, machine_layout) in native_plan.layouts.machine_layouts.iter() {
        storage.count_program_name_identity(&machine_layout.name);
    }
}

pub(in crate::identity) fn count_instruction_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, function) in native_plan.instructions.functions.iter() {
        storage.count_generated_symbol(&function.symbol);
        storage.count_program_name_identity(&function.machine);
        storage.count_program_name_identity(&function.state);
    }
    for (_, instruction) in native_plan.instructions.instructions.iter() {
        storage.count_program_name_identity(&instruction.source_machine);
        storage.count_program_name_identity(&instruction.source_state);
    }
}

pub(in crate::identity) fn count_object_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    storage.count_generated_symbol(&native_plan.object.entry_symbol);
    for (_, section) in native_plan.object.sections.iter() {
        storage.count_generated_symbol(&section.name);
    }
    for (_, symbol) in native_plan.object.symbols.iter() {
        storage.count_generated_symbol(&symbol.name);
        if let Some(section) = &symbol.section {
            storage.count_generated_symbol(section);
        }
    }
}

pub(in crate::identity) fn count_phase_timing_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for timing in &native_plan.phase_timings {
        storage.count_report(&timing.phase);
    }
}
