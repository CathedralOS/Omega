use crate::control_flow::{OperationKind, PlannedTransitionTarget};
use crate::host_calls::HostCallArgumentKind;
use crate::plan::NativePlan;
use crate::runtime_flow::RuntimeTransitionTarget;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeStringStorage {
    pub identity_strings: usize,
    pub identity_bytes: usize,
    pub payload_strings: usize,
    pub payload_bytes: usize,
    pub generated_symbol_strings: usize,
    pub generated_symbol_bytes: usize,
    pub report_strings: usize,
    pub report_bytes: usize,
}

impl NativeStringStorage {
    pub fn total_strings(self) -> usize {
        self.identity_strings
            + self.payload_strings
            + self.generated_symbol_strings
            + self.report_strings
    }

    pub fn total_bytes(self) -> usize {
        self.identity_bytes + self.payload_bytes + self.generated_symbol_bytes + self.report_bytes
    }

    fn count_identity(&mut self, value: &str) {
        count_string(&mut self.identity_strings, &mut self.identity_bytes, value);
    }

    fn count_payload(&mut self, value: &str) {
        count_string(&mut self.payload_strings, &mut self.payload_bytes, value);
    }

    fn count_generated_symbol(&mut self, value: &str) {
        count_string(
            &mut self.generated_symbol_strings,
            &mut self.generated_symbol_bytes,
            value,
        );
    }

    fn count_report(&mut self, value: &str) {
        count_string(&mut self.report_strings, &mut self.report_bytes, value);
    }

    fn count_program_name_identity(&mut self, name: &ProgramName) {
        if !name.is_source_backed() && !name.as_str().is_empty() {
            self.count_identity(name.as_str());
        }
    }
}

pub fn count_native_string_storage(native_plan: &NativePlan) -> NativeStringStorage {
    let mut storage = NativeStringStorage::default();

    storage.count_identity(&native_plan.entry_machine);
    storage.count_identity(&native_plan.entry_state);

    count_control_flow_strings(native_plan, &mut storage);
    count_runtime_flow_strings(native_plan, &mut storage);
    count_state_dispatch_strings(native_plan, &mut storage);
    count_host_call_strings(native_plan, &mut storage);
    count_state_call_strings(native_plan, &mut storage);
    count_state_storage_strings(native_plan, &mut storage);
    count_state_value_strings(native_plan, &mut storage);
    count_runtime_text_strings(native_plan, &mut storage);
    count_layout_strings(native_plan, &mut storage);
    count_instruction_strings(native_plan, &mut storage);
    count_object_strings(native_plan, &mut storage);
    count_phase_timing_strings(native_plan, &mut storage);

    storage
}

fn count_control_flow_strings(native_plan: &NativePlan, storage: &mut NativeStringStorage) {
    for (_, machine) in native_plan.control_flow.machines.iter() {
        storage.count_program_name_identity(&machine.name);
        for contained in &machine.contains {
            storage.count_program_name_identity(&contained.name);
            storage.count_program_name_identity(&contained.type_name);
        }
    }

    for (_, state) in native_plan.control_flow.states.iter() {
        storage.count_program_name_identity(&state.name);
        for parameter in &state.parameters {
            storage.count_program_name_identity(parameter);
        }
    }

    for (_, operation) in native_plan.control_flow.operations.iter() {
        match &operation.kind {
            OperationKind::Assignment { target, value }
            | OperationKind::StaticAssignment { target, value } => {
                count_expression_strings(target, storage);
                count_expression_strings(value, storage);
            }
            OperationKind::Call {
                receiver: _,
                target: _,
                arguments,
            } => {
                for argument in arguments {
                    count_expression_strings(argument, storage);
                }
            }
            OperationKind::ConstantIntegerAssignment
            | OperationKind::Expression
            | OperationKind::LocalData => {}
        }
    }

    for (_, transition) in native_plan.control_flow.transitions.iter() {
        count_planned_target_strings(&transition.target, storage);
        if let Some(continuation) = &transition.continuation {
            count_planned_target_strings(continuation, storage);
        }
    }
}

fn count_runtime_flow_strings(native_plan: &NativePlan, storage: &mut NativeStringStorage) {
    for (_, state) in native_plan.runtime_flow.states.iter() {
        storage.count_program_name_identity(&state.machine);
        storage.count_program_name_identity(&state.state);
    }
    for (_, edge) in native_plan.runtime_flow.edges.iter() {
        storage.count_program_name_identity(&edge.from_machine);
        storage.count_program_name_identity(&edge.from_state);
        count_runtime_target_strings(&edge.target, storage);
        count_runtime_target_strings(&edge.continuation, storage);
    }
    for (_, state) in native_plan.runtime_flow.cycle_states.iter() {
        storage.count_program_name_identity(&state.machine);
        storage.count_program_name_identity(&state.state);
    }
}

fn count_state_dispatch_strings(native_plan: &NativePlan, storage: &mut NativeStringStorage) {
    for (_, state) in native_plan.state_dispatch.states.iter() {
        storage.count_program_name_identity(&state.machine);
        storage.count_program_name_identity(&state.state);
        storage.count_generated_symbol(&state.label);
    }
    for (_, edge) in native_plan.state_dispatch.edges.iter() {
        count_runtime_target_strings(&edge.target, storage);
        count_runtime_target_strings(&edge.continuation, storage);
    }
}

fn count_host_call_strings(native_plan: &NativePlan, storage: &mut NativeStringStorage) {
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

fn count_state_call_strings(native_plan: &NativePlan, storage: &mut NativeStringStorage) {
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

fn count_state_storage_strings(native_plan: &NativePlan, storage: &mut NativeStringStorage) {
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

fn count_state_value_strings(native_plan: &NativePlan, storage: &mut NativeStringStorage) {
    for (_, value) in native_plan.state_values.values.iter() {
        storage.count_program_name_identity(&value.machine);
        storage.count_program_name_identity(&value.state);
        count_expression_strings(&value.expression, storage);
    }
}

fn count_runtime_text_strings(native_plan: &NativePlan, storage: &mut NativeStringStorage) {
    for (_, text_use) in native_plan.runtime_text.uses.iter() {
        storage.count_identity(&text_use.machine);
        storage.count_identity(&text_use.state);
        storage.count_identity(&text_use.platform_call);
        count_expression_strings(&text_use.expression, storage);
    }
    for (_, buffer) in native_plan.runtime_text.buffers.iter() {
        storage.count_identity(&buffer.machine);
        storage.count_identity(&buffer.state);
        storage.count_identity(&buffer.platform_call);
        count_expression_strings(&buffer.target, storage);
    }
    for (_, slot) in native_plan.runtime_text.slots.iter() {
        count_expression_strings(&slot.place, storage);
    }
    for (_, write) in native_plan.runtime_text.writes.iter() {
        storage.count_identity(&write.machine);
        storage.count_identity(&write.state);
        count_expression_strings(&write.target, storage);
        count_expression_strings(&write.value, storage);
    }
    for (_, builder) in native_plan.runtime_text.builders.iter() {
        storage.count_identity(&builder.machine);
        storage.count_identity(&builder.state);
        count_expression_strings(&builder.target, storage);
    }
    for (_, segment) in native_plan.runtime_text.builder_segments.iter() {
        count_expression_strings(&segment.expression, storage);
    }
}

fn count_layout_strings(native_plan: &NativePlan, storage: &mut NativeStringStorage) {
    for (_, data_layout) in native_plan.layouts.data_layouts.iter() {
        storage.count_identity(&data_layout.name);
        if let crate::layout::DataShape::Enum { variants } = &data_layout.shape {
            for variant in variants {
                storage.count_identity(variant);
            }
        }
    }
    for (_, field) in native_plan.layouts.fields.iter() {
        storage.count_identity(&field.name);
        storage.count_identity(&field.type_name);
    }
    for (_, machine_layout) in native_plan.layouts.machine_layouts.iter() {
        storage.count_identity(&machine_layout.name);
    }
}

fn count_instruction_strings(native_plan: &NativePlan, storage: &mut NativeStringStorage) {
    for (_, function) in native_plan.instructions.functions.iter() {
        storage.count_generated_symbol(&function.symbol);
        storage.count_identity(&function.machine);
        storage.count_identity(&function.state);
    }
    for (_, instruction) in native_plan.instructions.instructions.iter() {
        storage.count_identity(&instruction.source_machine);
        storage.count_identity(&instruction.source_state);
    }
}

fn count_object_strings(native_plan: &NativePlan, storage: &mut NativeStringStorage) {
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

fn count_phase_timing_strings(native_plan: &NativePlan, storage: &mut NativeStringStorage) {
    for timing in &native_plan.phase_timings {
        storage.count_report(&timing.phase);
    }
}

fn count_planned_target_strings(
    target: &PlannedTransitionTarget,
    storage: &mut NativeStringStorage,
) {
    match target {
        PlannedTransitionTarget::State {
            name, arguments, ..
        } => {
            storage.count_program_name_identity(name);
            for argument in arguments {
                count_expression_strings(argument, storage);
            }
        }
        PlannedTransitionTarget::Nested {
            receiver,
            state,
            arguments,
        } => {
            storage.count_program_name_identity(receiver);
            storage.count_program_name_identity(state);
            for argument in arguments {
                count_expression_strings(argument, storage);
            }
        }
        PlannedTransitionTarget::SelfTarget | PlannedTransitionTarget::Terminal => {}
    }
}

fn count_runtime_target_strings(
    target: &RuntimeTransitionTarget,
    storage: &mut NativeStringStorage,
) {
    match target {
        RuntimeTransitionTarget::State { machine, state } => {
            storage.count_program_name_identity(machine);
            storage.count_program_name_identity(state);
        }
        RuntimeTransitionTarget::Unknown { name } => storage.count_identity(name),
        RuntimeTransitionTarget::Terminal | RuntimeTransitionTarget::None => {}
    }
}

fn count_expression_strings(expression: &Expression, storage: &mut NativeStringStorage) {
    match expression {
        Expression::ArrayLiteral(values) => {
            for value in values {
                count_expression_strings(value, storage);
            }
        }
        Expression::Binary(binary) => {
            count_expression_strings(&binary.left, storage);
            count_expression_strings(&binary.right, storage);
        }
        Expression::Indexed(indexed) => {
            count_expression_strings(&indexed.collection, storage);
            count_expression_strings(&indexed.index, storage);
        }
        Expression::Mutable(expression) => count_expression_strings(expression, storage),
        Expression::StructLiteral(struct_literal) => {
            storage.count_program_name_identity(&struct_literal.type_name);
            for field in &struct_literal.fields {
                storage.count_program_name_identity(&field.name);
                count_expression_strings(&field.value, storage);
            }
        }
        Expression::Name(path) => {
            for name in path {
                storage.count_program_name_identity(name);
            }
        }
        Expression::String(value) => storage.count_payload(value),
        Expression::Boolean(_) | Expression::Float(_) | Expression::Integer(_) => {}
    }
}

fn count_string(count: &mut usize, bytes: &mut usize, value: &str) {
    *count += 1;
    *bytes += value.len();
}
