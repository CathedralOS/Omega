use crate::native::control_flow::OperationKind;
use crate::native::data::NativeDataObject;
use crate::native::host_calls::HostCall;
use crate::native::host_calls::{HostCallArgument, HostCallArgumentKind};
use crate::native::plan::NativePlan;
use crate::native::runtime_text::RuntimeTextSource;
use crate::native::state_schedule::build_entry_state_schedule;
use crate::native::target::{NativeTarget, ObjectFormat};
use omega_core::arena::{Arena, HandleSpan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionPlan {
    pub target: NativeTarget,
    pub functions: Arena<FunctionInstructionPlan>,
    pub instructions: Arena<SelectedInstruction>,
    pub operands: Arena<InstructionOperand>,
}

impl Default for InstructionPlan {
    fn default() -> Self {
        Self {
            target: NativeTarget::host(),
            functions: Arena::new(),
            instructions: Arena::new(),
            operands: Arena::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionInstructionPlan {
    pub symbol: String,
    pub machine: String,
    pub state: String,
    pub instructions: HandleSpan<SelectedInstruction>,
}

impl Default for FunctionInstructionPlan {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            machine: String::new(),
            state: String::new(),
            instructions: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedInstruction {
    pub kind: SelectedInstructionKind,
    pub source_machine: String,
    pub source_state: String,
    pub source_statement: usize,
}

impl Default for SelectedInstruction {
    fn default() -> Self {
        Self {
            kind: SelectedInstructionKind::EnterFunction,
            source_machine: String::new(),
            source_state: String::new(),
            source_statement: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedInstructionKind {
    EnterFunction,
    BeginPlatformCall {
        platform_call: String,
    },
    HostOperation {
        capability: String,
        operation: String,
        operands: HandleSpan<InstructionOperand>,
    },
    LeaveFunction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionOperand {
    pub kind: InstructionOperandKind,
}

impl Default for InstructionOperand {
    fn default() -> Self {
        Self {
            kind: InstructionOperandKind::ImmediateInteger(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstructionOperandKind {
    DataAddress { symbol: String },
    ImmediateInteger(i64),
    ByteLength(usize),
}

pub fn build_instruction_plan(native_plan: &NativePlan) -> InstructionPlan {
    let mut instruction_plan = InstructionPlan {
        target: native_plan.target,
        functions: Arena::new(),
        instructions: Arena::new(),
        operands: Arena::new(),
    };

    let entry_instructions = select_entry_instructions(native_plan, &mut instruction_plan.operands);
    let instructions = instruction_plan
        .instructions
        .insert_many(entry_instructions);

    instruction_plan.functions.insert(FunctionInstructionPlan {
        symbol: native_plan.object.entry_symbol.clone(),
        machine: native_plan.entry_machine.clone(),
        state: native_plan.entry_state.clone(),
        instructions,
    });

    instruction_plan
}

fn select_entry_instructions(
    native_plan: &NativePlan,
    operands: &mut Arena<InstructionOperand>,
) -> Vec<SelectedInstruction> {
    let mut selected_instructions = Vec::new();
    let state_schedule_result = build_entry_state_schedule(native_plan);
    let can_inline_state_calls = state_schedule_result.is_ok()
        && native_plan
            .state_calls
            .calls
            .iter()
            .any(|(_, call)| call.required);
    let state_schedule =
        state_schedule_result.unwrap_or_else(|_| runtime_reachable_states(native_plan));

    selected_instructions.push(entry_instruction(native_plan));

    if can_inline_state_calls {
        select_state_body_instructions(
            native_plan,
            &native_plan.entry_machine,
            &native_plan.entry_state,
            operands,
            &mut selected_instructions,
            &mut Vec::new(),
        );
    } else {
        for scheduled_state in &state_schedule {
            select_state_host_calls(
                native_plan,
                &scheduled_state.machine,
                &scheduled_state.state,
                operands,
                &mut selected_instructions,
            );
        }
    }

    selected_instructions.push(exit_instruction(native_plan));
    selected_instructions
}

fn select_state_body_instructions(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
    operands: &mut Arena<InstructionOperand>,
    selected_instructions: &mut Vec<SelectedInstruction>,
    visiting: &mut Vec<(String, String)>,
) {
    if visiting
        .iter()
        .any(|(machine, state)| machine == machine_name && state == state_name)
    {
        return;
    }

    visiting.push((machine_name.to_owned(), state_name.to_owned()));

    let Some(machine) = native_plan
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == machine_name)
        .map(|(_, machine)| machine)
    else {
        visiting.pop();
        return;
    };
    let Some(state) = native_plan
        .control_flow
        .states
        .span(machine.states)
        .and_then(|states| states.iter().find(|state| state.name == state_name))
    else {
        visiting.pop();
        return;
    };
    let Some(operations) = native_plan.control_flow.operations.span(state.operations) else {
        visiting.pop();
        return;
    };

    for operation in operations {
        if let Some(host_call) = host_call_for_statement(
            native_plan,
            machine_name,
            state_name,
            operation.statement_index,
        ) {
            select_host_call(native_plan, host_call, operands, selected_instructions);
            continue;
        }

        let OperationKind::Call { .. } = &operation.kind else {
            continue;
        };
        let Some(state_call) = state_call_for_statement(
            native_plan,
            machine_name,
            state_name,
            operation.statement_index,
        ) else {
            continue;
        };

        if state_call.target_machine.is_empty() {
            continue;
        }

        select_state_body_instructions(
            native_plan,
            &state_call.target_machine,
            &state_call.target_state,
            operands,
            selected_instructions,
            visiting,
        );
    }

    visiting.pop();
}

fn select_state_host_calls(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
    operands: &mut Arena<InstructionOperand>,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    for (_, host_call) in native_plan.host_calls.calls.iter() {
        if host_call.machine != machine_name || host_call.state != state_name {
            continue;
        }

        select_host_call(native_plan, host_call, operands, selected_instructions);
    }
}

fn host_call_for_statement<'plan>(
    native_plan: &'plan NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> Option<&'plan HostCall> {
    native_plan
        .host_calls
        .calls
        .iter()
        .find(|(_, host_call)| {
            host_call.machine == machine_name
                && host_call.state == state_name
                && host_call.statement_index == statement_index
        })
        .map(|(_, host_call)| host_call)
}

fn state_call_for_statement<'plan>(
    native_plan: &'plan NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> Option<&'plan crate::native::state_calls::StateCall> {
    native_plan
        .state_calls
        .calls
        .iter()
        .find(|(_, state_call)| {
            state_call.source_machine == machine_name
                && state_call.source_state == state_name
                && state_call.statement_index == statement_index
        })
        .map(|(_, state_call)| state_call)
}

fn runtime_reachable_states(
    native_plan: &NativePlan,
) -> Vec<crate::native::state_schedule::ScheduledState> {
    let mut states = Vec::new();

    for (_, state) in native_plan.runtime_flow.states.iter() {
        push_scheduled_state(&mut states, &state.machine, &state.state);
    }

    for (_, state_call) in native_plan.state_calls.calls.iter() {
        if !state_call.required {
            continue;
        }

        push_scheduled_state(
            &mut states,
            &state_call.source_machine,
            &state_call.source_state,
        );

        if !state_call.target_machine.is_empty() {
            push_scheduled_state(
                &mut states,
                &state_call.target_machine,
                &state_call.target_state,
            );
        }
    }

    states
}

fn push_scheduled_state(
    states: &mut Vec<crate::native::state_schedule::ScheduledState>,
    machine: &str,
    state: &str,
) {
    if states
        .iter()
        .any(|scheduled_state| scheduled_state.machine == machine && scheduled_state.state == state)
    {
        return;
    }

    states.push(crate::native::state_schedule::ScheduledState {
        machine: machine.to_owned(),
        state: state.to_owned(),
    });
}

fn select_host_call(
    native_plan: &NativePlan,
    host_call: &HostCall,
    operands: &mut Arena<InstructionOperand>,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::BeginPlatformCall {
            platform_call: host_call.platform_call.clone(),
        },
        source_machine: host_call.machine.clone(),
        source_state: host_call.state.clone(),
        source_statement: host_call.statement_index,
    });

    let Some(operations) = native_plan.host_calls.operations.span(host_call.operations) else {
        return;
    };

    for operation in operations {
        let operation_operands = select_host_operation_operands(
            native_plan,
            host_call,
            &operation.capability,
            &operation.operation,
        );
        let operation_operands = operands.insert_many(operation_operands);

        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::HostOperation {
                capability: operation.capability.clone(),
                operation: operation.operation.clone(),
                operands: operation_operands,
            },
            source_machine: host_call.machine.clone(),
            source_state: host_call.state.clone(),
            source_statement: host_call.statement_index,
        });
    }
}

fn select_host_operation_operands(
    native_plan: &NativePlan,
    host_call: &HostCall,
    capability: &str,
    operation: &str,
) -> Vec<InstructionOperand> {
    match (native_plan.target.object_format, capability, operation) {
        (ObjectFormat::Coff, "Stdout", "get_std_handle") => {
            vec![operand(InstructionOperandKind::ImmediateInteger(-11))]
        }
        (ObjectFormat::Coff, "Stdin", "get_std_handle") => {
            vec![operand(InstructionOperandKind::ImmediateInteger(-10))]
        }
        (_, "Stdin", "read" | "read_file") => {
            let Some(data_object) = find_data_object(native_plan, host_call) else {
                return Vec::new();
            };
            let byte_count = native_plan
                .data
                .bytes
                .span(data_object.bytes)
                .map_or(0, |bytes| bytes.len());

            let mut operands = Vec::new();
            if operation == "read" {
                operands.push(operand(InstructionOperandKind::ImmediateInteger(0)));
            }
            operands.push(operand(InstructionOperandKind::DataAddress {
                symbol: data_object.symbol.clone(),
            }));
            operands.push(operand(InstructionOperandKind::ByteLength(byte_count)));
            operands
        }
        (_, "Stdout", "write" | "write_file") => {
            let Some(data_object) = find_data_object(native_plan, host_call)
                .or_else(|| find_runtime_text_input_buffer_data_object(native_plan, host_call))
            else {
                return Vec::new();
            };
            let byte_count = native_plan
                .data
                .bytes
                .span(data_object.bytes)
                .map_or(0, |bytes| bytes.len());

            let mut operands = Vec::new();
            if operation == "write" {
                operands.push(operand(InstructionOperandKind::ImmediateInteger(1)));
            }
            operands.push(operand(InstructionOperandKind::DataAddress {
                symbol: data_object.symbol.clone(),
            }));
            operands.push(operand(InstructionOperandKind::ByteLength(byte_count)));
            operands
        }
        (_, "Process", "exit" | "exit_group" | "exit_process") => {
            vec![operand(InstructionOperandKind::ImmediateInteger(
                exit_code(host_call, native_plan),
            ))]
        }
        _ => Vec::new(),
    }
}

fn find_data_object<'plan>(
    native_plan: &'plan NativePlan,
    host_call: &HostCall,
) -> Option<&'plan NativeDataObject> {
    native_plan
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_machine == host_call.machine
                && data_object.source_state == host_call.state
                && data_object.source_statement == host_call.statement_index
        })
        .map(|(_, data_object)| data_object)
}

fn find_runtime_text_input_buffer_data_object<'plan>(
    native_plan: &'plan NativePlan,
    host_call: &HostCall,
) -> Option<&'plan NativeDataObject> {
    let text_use = native_plan
        .runtime_text
        .uses
        .iter()
        .find(|(_, text_use)| {
            text_use.machine == host_call.machine
                && text_use.state == host_call.state
                && text_use.statement_index == host_call.statement_index
                && text_use.platform_call == host_call.platform_call
                && text_use.source == RuntimeTextSource::StoredPlace
        })
        .map(|(_, text_use)| text_use)?;

    let text_slot = native_plan
        .runtime_text
        .slots
        .iter()
        .find(|(_, slot)| {
            slot.place.display_name() == text_use.expression.display_name() && slot.has_input_buffer
        })
        .map(|(_, slot)| slot)?;

    let buffer = native_plan
        .runtime_text
        .buffers
        .iter()
        .find(|(_, buffer)| {
            text_place_for_buffer_target(&buffer.target)
                .is_some_and(|place_name| place_name == text_slot.place.display_name())
        })
        .map(|(_, buffer)| buffer)?;

    native_plan
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_machine == buffer.machine
                && data_object.source_state == buffer.state
                && data_object.source_statement == buffer.statement_index
        })
        .map(|(_, data_object)| data_object)
}

fn text_place_for_buffer_target(target: &crate::ir::expression::Expression) -> Option<String> {
    match target {
        crate::ir::expression::Expression::Name(path) => {
            let mut text_path = path.clone();
            text_path.push("text".to_owned());
            Some(crate::ir::expression::Expression::Name(text_path).display_name())
        }
        _ => None,
    }
}

fn exit_code(host_call: &HostCall, native_plan: &NativePlan) -> i64 {
    first_argument(host_call, native_plan)
        .and_then(|argument| match &argument.kind {
            HostCallArgumentKind::Integer(value) => Some(*value),
            _ => None,
        })
        .unwrap_or(0)
}

fn first_argument<'plan>(
    host_call: &HostCall,
    native_plan: &'plan NativePlan,
) -> Option<&'plan HostCallArgument> {
    native_plan
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.first())
}

fn operand(kind: InstructionOperandKind) -> InstructionOperand {
    InstructionOperand { kind }
}

fn entry_instruction(native_plan: &NativePlan) -> SelectedInstruction {
    SelectedInstruction {
        kind: SelectedInstructionKind::EnterFunction,
        source_machine: native_plan.entry_machine.clone(),
        source_state: native_plan.entry_state.clone(),
        source_statement: 0,
    }
}

fn exit_instruction(native_plan: &NativePlan) -> SelectedInstruction {
    SelectedInstruction {
        kind: SelectedInstructionKind::LeaveFunction,
        source_machine: native_plan.entry_machine.clone(),
        source_state: native_plan.entry_state.clone(),
        source_statement: 0,
    }
}
