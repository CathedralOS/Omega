use crate::phase_diagram::PhaseDiagramBuilder;
use omega_abstract_operations::{AbstractOperation, AbstractOperationPlan};
use omega_assigned_target_operations::{AssignedOperation, AssignedTargetOperationPlan};
use omega_control_flow::{
    ControlFlowPlan, MachineFlow, Operation, OperationKind, PlannedTransitionTarget, StateFlow,
    StateKey, TransitionFlow,
};
use omega_machine_instructions::{MachineInstruction, MachineInstructionPlan};
use omega_target_operations::{TargetOperation, TargetOperationPlan};
use std::fmt::Debug;

pub fn abstract_operations_html(
    plan: &AbstractOperationPlan,
    control_flow: &ControlFlowPlan,
) -> String {
    build_backend_cfg_diagram(
        "abstract_operations",
        "Abstract Operations",
        control_flow,
        plan.functions.storage_slice(),
        |function| function.source_key,
        |_| "abstract block",
        |function| {
            numbered_lines(
                plan.instructions.span_or_empty(function.instructions),
                abstract_instruction_line,
            )
        },
    )
}

pub fn target_operations_html(
    plan: &TargetOperationPlan,
    control_flow: &ControlFlowPlan,
) -> String {
    build_backend_cfg_diagram(
        "target_operations",
        "Target Operations",
        control_flow,
        plan.functions.storage_slice(),
        |function| function.source_key,
        |_| "target block",
        |function| {
            numbered_lines(
                plan.instructions.span_or_empty(function.instructions),
                target_instruction_line,
            )
        },
    )
}

pub fn assigned_target_operations_html(
    plan: &AssignedTargetOperationPlan,
    control_flow: &ControlFlowPlan,
) -> String {
    build_backend_cfg_diagram(
        "assigned_target_operations",
        "Assigned Target Operations",
        control_flow,
        plan.functions.storage_slice(),
        |function| function.source_key,
        |_| "assigned block",
        |function| {
            numbered_lines(
                plan.instructions.span_or_empty(function.instructions),
                assigned_instruction_line,
            )
        },
    )
}

pub fn machine_instructions_html(
    plan: &MachineInstructionPlan,
    control_flow: &ControlFlowPlan,
) -> String {
    build_backend_cfg_diagram(
        "machine_instructions",
        &format!("Machine Instructions\n{:?}", plan.target),
        control_flow,
        plan.functions.storage_slice(),
        |function| function.source_key,
        |_| "machine block",
        |function| {
            numbered_lines(
                plan.instructions.span_or_empty(function.instructions),
                machine_instruction_line,
            )
        },
    )
}

fn build_backend_cfg_diagram<Function>(
    title: &str,
    _root_label: &str,
    control_flow: &ControlFlowPlan,
    functions: &[Function],
    function_source_key: impl Fn(&Function) -> StateKey,
    function_title: impl Fn(&Function) -> &str,
    function_lines: impl Fn(&Function) -> Vec<String>,
) -> String {
    let mut diagram = PhaseDiagramBuilder::new(title);

    let function_views = functions
        .iter()
        .map(|function| FunctionView {
            source_key: function_source_key(function),
            title: function_title(function).to_owned(),
            lines: function_lines(function),
        })
        .collect::<Vec<_>>();

    let mut state_nodes = Vec::<(StateKey, String)>::new();

    for (machine_index, (_, machine)) in control_flow.machines.iter().enumerate() {
        let machine_id = diagram.node(
            format!("machine_{}", machine.symbol.arena_index()),
            machine_backend_label(machine),
            "machine",
            machine_index + 1,
        );

        for state in unique_machine_states(control_flow, machine) {
            let function = function_view_by_key(&function_views, state.key);
            let state_id = diagram.node(
                format!(
                    "state_{}_{}_{}",
                    state.key.machine.arena_index(),
                    state.key.state.arena_index(),
                    state.key.segment_index
                ),
                state_backend_label(control_flow, state, function),
                "state_block",
                machine_index + 1,
            );
            diagram.node_details(
                &state_id,
                state_backend_details(control_flow, state, function),
            );
            diagram.containment_edge(&machine_id, &state_id);
            state_nodes.push((state.key, state_id));
        }
    }

    for (_, machine) in control_flow.machines.iter() {
        for state in unique_machine_states(control_flow, machine) {
            let Some(source_id) = state_node_id(&state_nodes, state.key) else {
                continue;
            };

            for transition in control_flow.transitions.span_or_empty(state.transitions) {
                append_transition_edges(
                    &mut diagram,
                    control_flow,
                    &state_nodes,
                    state.key,
                    source_id,
                    transition,
                );
            }

            for operation in control_flow.operations.span_or_empty(state.operations) {
                if let Some(target_key) = operation_call_target_key(control_flow, operation) {
                    if let Some(target_id) = state_node_id(&state_nodes, target_key) {
                        diagram.edge(source_id, target_id, "call");
                    }
                }
            }
        }
    }

    diagram.finish()
}

struct FunctionView {
    source_key: StateKey,
    title: String,
    lines: Vec<String>,
}

const BLOCK_PREVIEW_LINES: usize = 4;

fn function_view_by_key(functions: &[FunctionView], key: StateKey) -> Option<&FunctionView> {
    functions.iter().find(|function| function.source_key == key)
}

fn numbered_lines<Item>(items: &[Item], line: impl Fn(&Item) -> String) -> Vec<String> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| format!("{index:02} {}", line(item)))
        .collect()
}

fn machine_backend_label(machine: &MachineFlow) -> String {
    machine.name.to_string()
}

fn state_backend_label(
    control_flow: &ControlFlowPlan,
    state: &StateFlow,
    function: Option<&FunctionView>,
) -> String {
    let mut lines = vec![
        format!(
            "{} [{}]",
            state_scoped_name(control_flow, state.key),
            state.key.segment_index
        ),
        function_title(function),
        state_flow_summary(control_flow, state),
    ];

    if let Some(function) = function {
        if function.lines.is_empty() {
            lines.push("no instructions".to_owned());
        } else {
            lines.push(format!("instructions: {}", function.lines.len()));
            lines.extend(block_preview_lines(&function.lines));
        }
    }

    lines.join("\n")
}

fn state_backend_details(
    control_flow: &ControlFlowPlan,
    state: &StateFlow,
    function: Option<&FunctionView>,
) -> String {
    let mut lines = vec![
        format!(
            "{} [{}]",
            state_scoped_name(control_flow, state.key),
            state.key.segment_index
        ),
        function_title(function),
        state_flow_summary(control_flow, state),
    ];

    match function {
        Some(function) if !function.lines.is_empty() => {
            lines.push(format!("instructions: {}", function.lines.len()));
            lines.push(String::new());
            lines.extend(function.lines.iter().cloned());
        }
        Some(_) => lines.push("no instructions".to_owned()),
        None => lines.push("no lowered block".to_owned()),
    }

    lines.join("\n")
}

fn function_title(function: Option<&FunctionView>) -> String {
    function
        .map(|function| function.title.clone())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "no lowered block".to_owned())
}

fn state_flow_summary(control_flow: &ControlFlowPlan, state: &StateFlow) -> String {
    let call_count = control_flow
        .operations
        .span_or_empty(state.operations)
        .iter()
        .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .count();
    let transition_count = control_flow
        .transitions
        .span_or_empty(state.transitions)
        .len();
    format!("calls: {call_count} transitions: {transition_count}")
}

fn block_preview_lines(lines: &[String]) -> Vec<String> {
    let preview_count = lines.len().min(BLOCK_PREVIEW_LINES);
    let mut preview = lines
        .iter()
        .take(preview_count)
        .cloned()
        .collect::<Vec<_>>();
    if lines.len() > BLOCK_PREVIEW_LINES {
        preview.push(format!(
            "... {} more lines in details",
            lines.len() - BLOCK_PREVIEW_LINES
        ));
    }
    preview
}

fn state_scoped_name(control_flow: &ControlFlowPlan, key: StateKey) -> String {
    let machine_name = control_flow
        .machine_by_symbol(key.machine)
        .map(|machine| machine.name.as_str())
        .unwrap_or("unknown_machine");
    let state_name = control_flow
        .state_by_key(key)
        .map(|state| state.name.as_str())
        .unwrap_or("unknown_state");
    format!("{machine_name}::{state_name}")
}

fn state_node_id(state_nodes: &[(StateKey, String)], key: StateKey) -> Option<&str> {
    state_nodes
        .iter()
        .find(|(state_key, _)| *state_key == key)
        .map(|(_, id)| id.as_str())
}

fn unique_machine_states<'plan>(
    plan: &'plan ControlFlowPlan,
    machine: &MachineFlow,
) -> Vec<&'plan StateFlow> {
    let mut states = Vec::new();
    for state in plan.states.span_or_empty(machine.states) {
        if states
            .iter()
            .any(|existing: &&StateFlow| existing.key == state.key)
        {
            continue;
        }
        states.push(state);
    }
    states
}

fn append_transition_edges(
    diagram: &mut PhaseDiagramBuilder,
    control_flow: &ControlFlowPlan,
    state_nodes: &[(StateKey, String)],
    source_key: StateKey,
    source_id: &str,
    transition: &TransitionFlow,
) {
    if let Some(target_key) =
        transition_target_key(control_flow, &transition.target, Some(source_key))
    {
        if let Some(target_id) = state_node_id(state_nodes, target_key) {
            diagram.edge(source_id, target_id, "transition_target");
        }
    }

    if let Some(target_key) =
        transition_target_key(control_flow, &transition.continuation, Some(source_key))
    {
        if let Some(target_id) = state_node_id(state_nodes, target_key) {
            diagram.edge(source_id, target_id, "transition_continuation");
        }
    }
}

fn transition_target_key(
    control_flow: &ControlFlowPlan,
    target: &PlannedTransitionTarget,
    source_key: Option<StateKey>,
) -> Option<StateKey> {
    match target {
        PlannedTransitionTarget::None | PlannedTransitionTarget::Terminal => None,
        PlannedTransitionTarget::SelfTarget => source_key,
        PlannedTransitionTarget::State { key, .. } => Some(*key),
        PlannedTransitionTarget::Nested {
            receiver_symbol,
            state_symbol,
            ..
        } => control_flow.state_key_by_symbols(*receiver_symbol, *state_symbol),
    }
}

fn operation_call_target_key(
    control_flow: &ControlFlowPlan,
    operation: &Operation,
) -> Option<StateKey> {
    let OperationKind::Call {
        receiver_symbol,
        target_symbol,
        has_receiver,
        ..
    } = operation.kind
    else {
        return None;
    };

    if has_receiver {
        return control_flow.state_key_by_symbols(receiver_symbol, target_symbol);
    }

    control_flow
        .states
        .iter()
        .find(|(_, state)| state.key.state == target_symbol)
        .map(|(_, state)| state.key)
}

fn abstract_instruction_line(instruction: &AbstractOperation) -> String {
    format!(
        "{} @ statement {}",
        enum_variant_name(&instruction.kind),
        instruction.source_statement
    )
}

fn target_instruction_line(instruction: &TargetOperation) -> String {
    format!(
        "{} @ statement {}",
        enum_variant_name(&instruction.kind),
        instruction.source_statement
    )
}

fn assigned_instruction_line(instruction: &AssignedOperation) -> String {
    format!(
        "{} @ statement {}",
        enum_variant_name(&instruction.kind),
        instruction.source_statement
    )
}

fn machine_instruction_line(instruction: &MachineInstruction) -> String {
    format!(
        "{:?} <- {} #{}",
        instruction.kind,
        enum_variant_name(&instruction.source_kind),
        instruction.selected_instruction_index
    )
}

fn enum_variant_name(value: &impl Debug) -> String {
    let debug = format!("{value:?}");
    let end = debug.find([' ', '{', '(']).unwrap_or(debug.len());
    debug[..end].to_owned()
}
