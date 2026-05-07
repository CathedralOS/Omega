use crate::ir::expression::Expression;
use crate::native::control_flow::OperationKind;
use crate::native::data::NativeDataObject;
use crate::native::host_calls::HostCall;
use crate::native::host_calls::{HostCallArgument, HostCallArgumentKind};
use crate::native::layout::{DataShape, FieldLayout, LayoutPlan, TypeLayout};
use crate::native::plan::NativePlan;
use crate::native::runtime_dispatch::bodies::RuntimeDispatchBodyOperationKind;
use crate::native::runtime_dispatch::branching::{
    RuntimeLeafBranchBinding, RuntimeLeafBranchBindingKind, RuntimeLeafBranchExpansion,
    RuntimeLeafBranchOperationKind,
};
use crate::native::runtime_dispatch::loop_plan::{
    RuntimeDispatchLoopAction, RuntimeDispatchLoopEdge,
};
use crate::native::runtime_text::{RuntimeTextSource, RuntimeTextWriteKind};
use crate::native::state_guards::StateGuardLowering;
use crate::native::state_guards::StateGuardOperator;
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
    EnterDispatchLoop {
        entry_dispatch_index: u32,
        terminal_dispatch_index: u32,
        current_state_slot: String,
        next_state_slot: String,
    },
    EnterDispatchCase {
        dispatch_index: u32,
        label: String,
    },
    EvaluateDispatchGuard {
        guard_lowering: StateGuardLowering,
        operator: StateGuardOperator,
        byte_offset: usize,
        byte_size: usize,
        expected_value: i64,
        has_storage: bool,
    },
    CompareRuntimeTextLiteral {
        buffer_symbol: String,
        literal: String,
    },
    WriteRuntimeTextLiteral {
        buffer_symbol: String,
        literal: String,
    },
    WriteRuntimeMachineInteger {
        byte_offset: usize,
        byte_size: usize,
        value: i64,
    },
    SetDispatchState {
        dispatch_index: u32,
    },
    TerminateDispatch,
    LeaveDispatchCase,
    LeaveDispatchLoop,
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

    if native_plan.runtime_dispatch_loop.needed {
        select_runtime_dispatch_loop_instructions(
            native_plan,
            operands,
            &mut selected_instructions,
        );
    } else if can_inline_state_calls {
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

fn select_runtime_dispatch_loop_instructions(
    native_plan: &NativePlan,
    operands: &mut Arena<InstructionOperand>,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index: native_plan.runtime_dispatch_loop.entry_dispatch_index,
            terminal_dispatch_index: native_plan.runtime_dispatch_loop.terminal_dispatch_index,
            current_state_slot: native_plan.runtime_dispatch_loop.current_state_slot.clone(),
            next_state_slot: native_plan.runtime_dispatch_loop.next_state_slot.clone(),
        },
        source_machine: native_plan.entry_machine.clone(),
        source_state: native_plan.entry_state.clone(),
        source_statement: 0,
    });

    for (_, dispatch_case) in native_plan.runtime_dispatch_loop.cases.iter() {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::EnterDispatchCase {
                dispatch_index: dispatch_case.dispatch_index,
                label: dispatch_case.label.clone(),
            },
            source_machine: dispatch_case.machine.clone(),
            source_state: dispatch_case.state.clone(),
            source_statement: 0,
        });

        if let Some(runtime_body) = native_plan
            .runtime_bodies
            .bodies
            .iter()
            .find(|(_, body)| body.dispatch_index == dispatch_case.dispatch_index)
            .map(|(_, body)| body)
            && let Some(operations) = native_plan
                .runtime_bodies
                .operations
                .span(runtime_body.operations)
        {
            for operation in operations {
                if let Some(host_call) = host_call_for_statement(
                    native_plan,
                    &operation.source_machine,
                    &operation.source_state,
                    operation.statement_index,
                ) {
                    if let Some((buffer_symbol, literal)) =
                        runtime_text_literal_write_for_host_call(native_plan, host_call)
                    {
                        selected_instructions.push(SelectedInstruction {
                            kind: SelectedInstructionKind::WriteRuntimeTextLiteral {
                                buffer_symbol,
                                literal,
                            },
                            source_machine: host_call.machine.clone(),
                            source_state: host_call.state.clone(),
                            source_statement: host_call.statement_index,
                        });
                    }
                    select_host_call(native_plan, host_call, operands, selected_instructions);
                }
            }
        }

        select_runtime_leaf_branch_expansions(
            native_plan,
            dispatch_case.dispatch_index,
            selected_instructions,
        );

        if let Some(edges) = native_plan
            .runtime_dispatch_loop
            .edges
            .span(dispatch_case.edges)
        {
            for edge in edges {
                select_runtime_dispatch_edge(
                    edge,
                    &dispatch_case.machine,
                    &dispatch_case.state,
                    selected_instructions,
                );
            }
        }

        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::LeaveDispatchCase,
            source_machine: dispatch_case.machine.clone(),
            source_state: dispatch_case.state.clone(),
            source_statement: 0,
        });
    }

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::LeaveDispatchLoop,
        source_machine: native_plan.entry_machine.clone(),
        source_state: native_plan.entry_state.clone(),
        source_statement: 0,
    });
}

fn select_runtime_dispatch_edge(
    edge: &RuntimeDispatchLoopEdge,
    source_machine: &str,
    source_state: &str,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: edge.guard_lowering,
            operator: edge.guard_operator,
            byte_offset: edge.guard_byte_offset,
            byte_size: edge.guard_byte_size,
            expected_value: edge.guard_expected_value,
            has_storage: edge.guard_has_storage,
        },
        source_machine: source_machine.to_owned(),
        source_state: source_state.to_owned(),
        source_statement: edge.order,
    });

    match edge.action {
        RuntimeDispatchLoopAction::EnterState => {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::SetDispatchState {
                    dispatch_index: edge.target_dispatch_index,
                },
                source_machine: source_machine.to_owned(),
                source_state: source_state.to_owned(),
                source_statement: edge.order,
            });
        }
        RuntimeDispatchLoopAction::Terminate => {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::TerminateDispatch,
                source_machine: source_machine.to_owned(),
                source_state: source_state.to_owned(),
                source_statement: edge.order,
            });
        }
        RuntimeDispatchLoopAction::Unknown => {}
    }
}

fn select_runtime_leaf_branch_expansions(
    native_plan: &NativePlan,
    dispatch_index: u32,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    for (_, expansion) in native_plan
        .runtime_branching_calls
        .leaf_expansions
        .iter()
        .filter(|(_, expansion)| expansion.dispatch_index == dispatch_index)
    {
        let Some((buffer_symbol, literal)) = runtime_text_literal_guard(native_plan, expansion)
        else {
            continue;
        };
        let Some((byte_offset, byte_size, value)) =
            runtime_leaf_machine_integer_write(native_plan, expansion)
        else {
            continue;
        };

        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::CompareRuntimeTextLiteral {
                buffer_symbol,
                literal,
            },
            source_machine: expansion.source_machine.clone(),
            source_state: expansion.source_state.clone(),
            source_statement: expansion.statement_index,
        });
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteRuntimeMachineInteger {
                byte_offset,
                byte_size,
                value,
            },
            source_machine: expansion.leaf_machine.clone(),
            source_state: expansion.leaf_state.clone(),
            source_statement: expansion.statement_index,
        });
    }
}

fn runtime_text_literal_guard(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
) -> Option<(String, String)> {
    let crate::ir::statement::TransitionGuard::When(Expression::Binary(binary)) =
        &expansion.resolved_guard
    else {
        return None;
    };
    if binary.operator != crate::ir::expression::BinaryOperator::Equal {
        return None;
    }

    let (text_place, literal) = match (&binary.left, &binary.right) {
        (text_place, Expression::String(literal)) => (text_place, literal),
        (Expression::String(literal), text_place) => (text_place, literal),
        _ => return None,
    };

    let buffer = runtime_text_input_buffer_for_text_place(native_plan, text_place)?;
    Some((buffer.symbol.clone(), literal.clone()))
}

fn runtime_leaf_machine_integer_write(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
) -> Option<(usize, usize, i64)> {
    let operations = native_plan
        .runtime_branching_calls
        .leaf_operations
        .span(expansion.operations)?;
    let operation = operations.iter().find_map(|operation| {
        let RuntimeLeafBranchOperationKind::Mutation { target, value, .. } = &operation.kind else {
            return None;
        };
        Some((target, value))
    })?;
    let bindings = native_plan
        .runtime_branching_calls
        .leaf_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);
    let target = resolve_leaf_binding_expression(operation.0, bindings);
    let (byte_offset, byte_size) = resolve_machine_owned_place(
        &native_plan.layouts,
        &native_plan.entry_machine,
        &expansion.source_machine,
        &target,
    )?;
    let value = enum_variant_value(&native_plan.layouts, operation.1)?;

    Some((byte_offset, byte_size, value))
}

fn resolve_leaf_binding_expression(
    expression: &Expression,
    bindings: &[RuntimeLeafBranchBinding],
) -> Expression {
    match expression {
        Expression::Mutable(target) => {
            let resolved_target = resolve_leaf_binding_expression(target, bindings);
            if matches!(resolved_target, Expression::Mutable(_)) {
                resolved_target
            } else {
                Expression::Mutable(Box::new(resolved_target))
            }
        }
        Expression::Name(path) if !path.is_empty() => bindings
            .iter()
            .find(|binding| {
                binding.parameter_name == path[0]
                    && binding.kind == RuntimeLeafBranchBindingKind::LeafParameter
            })
            .or_else(|| {
                bindings
                    .iter()
                    .find(|binding| binding.parameter_name == path[0])
            })
            .map(|binding| append_place_suffix(&binding.expression, &path[1..]))
            .unwrap_or_else(|| expression.clone()),
        _ => expression.clone(),
    }
}

fn append_place_suffix(expression: &Expression, suffix: &[String]) -> Expression {
    if suffix.is_empty() {
        return expression.clone();
    }

    match expression {
        Expression::Name(path) => {
            let mut resolved_path = path.clone();
            resolved_path.extend_from_slice(suffix);
            Expression::Name(resolved_path)
        }
        Expression::Mutable(target) => {
            Expression::Mutable(Box::new(append_place_suffix(target, suffix)))
        }
        _ => expression.clone(),
    }
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
            let (data_object, byte_count) =
                if let Some(data_object) = find_data_object(native_plan, host_call) {
                    let byte_count = native_plan
                        .data
                        .bytes
                        .span(data_object.bytes)
                        .map_or(0, |bytes| bytes.len());
                    (data_object, byte_count)
                } else if let Some(data_object) =
                    find_runtime_text_input_buffer_data_object(native_plan, host_call)
                {
                    let byte_count = runtime_text_literal_for_host_call(native_plan, host_call)
                        .map(|literal| literal.len())
                        .unwrap_or(0);
                    (data_object, byte_count)
                } else {
                    return Vec::new();
                };

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

fn runtime_text_input_buffer_for_text_place<'plan>(
    native_plan: &'plan NativePlan,
    text_place: &Expression,
) -> Option<&'plan NativeDataObject> {
    let text_place_name = text_place.display_name();
    let buffer = native_plan
        .runtime_text
        .buffers
        .iter()
        .find_map(|(_, buffer)| {
            text_place_for_buffer_target(&buffer.target)
                .is_some_and(|place_name| place_name == text_place_name)
                .then_some(buffer)
        })?;

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

fn runtime_text_literal_write_for_host_call(
    native_plan: &NativePlan,
    host_call: &HostCall,
) -> Option<(String, String)> {
    let literal = runtime_text_literal_for_host_call(native_plan, host_call)?;
    let data_object = find_runtime_text_input_buffer_data_object(native_plan, host_call)?;
    Some((data_object.symbol.clone(), literal))
}

fn runtime_text_literal_for_host_call(
    native_plan: &NativePlan,
    host_call: &HostCall,
) -> Option<String> {
    let append_newline = match host_call.data {
        crate::native::abi::PlatformCallData::FirstTextArgument { append_newline } => {
            append_newline
        }
        crate::native::abi::PlatformCallData::MutableOutputBuffer { .. }
        | crate::native::abi::PlatformCallData::None => return None,
    };
    if !host_call_uses_runtime_text_input_buffer(native_plan, host_call) {
        return None;
    }

    let runtime_body = native_plan
        .runtime_bodies
        .bodies
        .iter()
        .find(|(_, body)| {
            native_plan
                .runtime_bodies
                .operations
                .span(body.operations)
                .is_some_and(|operations| {
                    operations.iter().any(|operation| {
                        operation.source_machine == host_call.machine
                            && operation.source_state == host_call.state
                            && operation.statement_index == host_call.statement_index
                            && matches!(
                                operation.kind,
                                RuntimeDispatchBodyOperationKind::HostCall { .. }
                            )
                    })
                })
        })
        .map(|(_, body)| body)?;
    let operations = native_plan
        .runtime_bodies
        .operations
        .span(runtime_body.operations)?;
    let mut latest_static_text = None;

    for operation in operations {
        if operation.source_machine == host_call.machine
            && operation.source_state == host_call.state
            && operation.statement_index == host_call.statement_index
            && matches!(
                operation.kind,
                RuntimeDispatchBodyOperationKind::HostCall { .. }
            )
        {
            break;
        }

        let Some(text_write) = runtime_text_write_for_operation(
            native_plan,
            &operation.source_machine,
            &operation.source_state,
            operation.statement_index,
        ) else {
            continue;
        };
        if text_write.kind != RuntimeTextWriteKind::StaticText {
            continue;
        }
        let Expression::String(value) = &text_write.value else {
            continue;
        };
        latest_static_text = Some(value.clone());
    }

    let mut literal = latest_static_text?;
    if append_newline {
        literal.push('\n');
    }
    Some(literal)
}

fn host_call_uses_runtime_text_input_buffer(
    native_plan: &NativePlan,
    host_call: &HostCall,
) -> bool {
    find_runtime_text_input_buffer_data_object(native_plan, host_call).is_some()
}

fn runtime_text_write_for_operation<'plan>(
    native_plan: &'plan NativePlan,
    machine: &str,
    state: &str,
    statement_index: usize,
) -> Option<&'plan crate::native::runtime_text::RuntimeTextWrite> {
    native_plan
        .runtime_text
        .writes
        .iter()
        .find(|(_, write)| {
            write.machine == machine
                && write.state == state
                && write.statement_index == statement_index
        })
        .map(|(_, write)| write)
}

fn resolve_machine_owned_place(
    layouts: &LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
    expression: &Expression,
) -> Option<(usize, usize)> {
    let expression = match expression {
        Expression::Mutable(target) => target.as_ref(),
        _ => expression,
    };
    let Expression::Name(path) = expression else {
        return None;
    };
    let [root_name, suffix @ ..] = path.as_slice() else {
        return None;
    };
    let machine_base_offset = machine_storage_offset(layouts, entry_machine, source_machine)?;
    let machine_layout = layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.name == source_machine)
        .map(|(_, machine_layout)| machine_layout)?;
    let root_field = field_layout(layouts, machine_layout.fields, root_name)?;
    let (field_offset, field_layout) = resolve_nested_field_layout(layouts, root_field, suffix)?;

    Some((machine_base_offset + field_offset, field_layout.size))
}

fn machine_storage_offset(
    layouts: &LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
) -> Option<usize> {
    if entry_machine == source_machine {
        return Some(0);
    }

    let entry_layout = layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.name == entry_machine)
        .map(|(_, machine_layout)| machine_layout)?;
    layouts
        .fields
        .span(entry_layout.fields)?
        .iter()
        .find(|field| field.type_name == source_machine)
        .map(|field| field.offset)
}

fn resolve_nested_field_layout(
    layouts: &LayoutPlan,
    root_field: &FieldLayout,
    suffix: &[String],
) -> Option<(usize, TypeLayout)> {
    let mut byte_offset = root_field.offset;
    let mut type_name = root_field.type_name.as_str();
    let mut layout = root_field.layout;

    for field_name in suffix {
        let data_layout = layouts
            .data_layouts
            .iter()
            .find(|(_, data_layout)| data_layout.name == type_name)
            .map(|(_, data_layout)| data_layout)?;
        let DataShape::Record { fields } = &data_layout.shape else {
            return None;
        };
        let field = field_layout(layouts, *fields, field_name)?;
        byte_offset += field.offset;
        type_name = &field.type_name;
        layout = field.layout;
    }

    Some((byte_offset, layout))
}

fn field_layout<'plan>(
    layouts: &'plan LayoutPlan,
    fields: HandleSpan<FieldLayout>,
    field_name: &str,
) -> Option<&'plan FieldLayout> {
    layouts
        .fields
        .span(fields)?
        .iter()
        .find(|field| field.name == field_name)
}

fn enum_variant_value(layouts: &LayoutPlan, expression: &Expression) -> Option<i64> {
    let Expression::Name(path) = expression else {
        return None;
    };
    let [type_name, variant_name] = path.as_slice() else {
        return None;
    };
    let data_layout = layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.name == *type_name)
        .map(|(_, data_layout)| data_layout)?;
    let DataShape::Enum { variants } = &data_layout.shape else {
        return None;
    };
    variants
        .iter()
        .position(|variant| variant == variant_name)
        .and_then(|index| i64::try_from(index).ok())
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
