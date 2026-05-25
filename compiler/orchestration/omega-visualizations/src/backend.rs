use crate::phase_diagram::PhaseDiagramBuilder;
use omega_abstract_operations::{AbstractOperation, AbstractOperationPlan};
use omega_assigned_target_operations::{AssignedOperation, AssignedTargetOperationPlan};
use omega_control_flow::{
    ControlFlowPlan, MachineFlow, Operation, OperationKind, PlannedTransitionTarget, StateFlow,
    StateKey, TransitionFlow,
};
use omega_core::symbols::SymbolHandle;
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
        |_| "abstract block".to_owned(),
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
        |_| "target block".to_owned(),
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
        |_| "assigned block".to_owned(),
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
    build_machine_instruction_diagram(plan, control_flow)
}

fn build_backend_cfg_diagram<Function>(
    title: &str,
    _root_label: &str,
    control_flow: &ControlFlowPlan,
    functions: &[Function],
    function_source_key: impl Fn(&Function) -> StateKey,
    function_title: impl Fn(&Function) -> String,
    function_lines: impl Fn(&Function) -> Vec<String>,
) -> String {
    let mut diagram = PhaseDiagramBuilder::new(title);
    let mut machine_nodes = Vec::new();
    let mut state_scope_nodes = Vec::new();
    let mut terminal_anchor_nodes = Vec::<(StateKey, String)>::new();

    let function_views = functions
        .iter()
        .map(|function| FunctionView {
            source_key: function_source_key(function),
            title: function_title(function),
            lines: function_lines(function),
        })
        .collect::<Vec<_>>();

    let mut state_nodes = Vec::<(StateKey, String)>::new();

    for (machine_index, (_, machine)) in control_flow.machines.iter().enumerate() {
        let states = unique_machine_states(control_flow, machine);
        let root_keys = backend_visual_root_keys(control_flow, &states);

        for root_key in &root_keys {
            let Some(root_state) = backend_state_by_key_in_slice(&states, *root_key) else {
                continue;
            };
            let machine_id = diagram.node(
                format!(
                    "machine_{}_{}",
                    machine.symbol.arena_index(),
                    root_key.state.arena_index()
                ),
                backend_machine_label(machine, root_state),
                "machine",
                machine_index + 1,
            );
            machine_nodes.push((machine.symbol, *root_key, machine_id));
        }

        for state in states.iter().copied() {
            let root_key = backend_root_key_for_state(control_flow, &states, &root_keys, state.key);
            let Some(machine_id) =
                backend_machine_id_for_root(&machine_nodes, machine.symbol, root_key)
            else {
                continue;
            };
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
            diagram.containment_edge(machine_id, &state_id);
            state_nodes.push((state.key, state_id.clone()));
            state_scope_nodes.push((state.key, machine_id.to_owned()));

            if let Some(function) = function {
                let chunks = operation_chunks(&function.lines);
                let mut parent_chunk_id = state_id.clone();
                let mut terminal_anchor_id = state_id.clone();
                for chunk in &chunks {
                    let chunk_id = diagram.node(
                        format!(
                            "operation_chunk_{}_{}_{}_{}",
                            state.key.machine.arena_index(),
                            state.key.state.arena_index(),
                            state.key.segment_index,
                            chunk.index
                        ),
                        operation_chunk_label(chunk, &state_scoped_name(control_flow, state.key)),
                        "scoped_statement",
                        machine_index + 1,
                    );
                    diagram.node_details(
                        &chunk_id,
                        operation_chunk_details(chunk, &state_scoped_name(control_flow, state.key)),
                    );
                    diagram.containment_edge(&parent_chunk_id, &chunk_id);
                    parent_chunk_id = chunk_id.clone();
                    terminal_anchor_id = chunk_id;
                }
                terminal_anchor_nodes.push((state.key, terminal_anchor_id));
            } else {
                terminal_anchor_nodes.push((state.key, state_id.clone()));
            }
        }
    }

    for (_, machine) in control_flow.machines.iter() {
        for state in control_flow.states.span_or_empty(machine.states) {
            let Some(source_state_id) = state_node_id(&state_nodes, state.key) else {
                continue;
            };
            let source_anchor_id =
                state_node_id(&terminal_anchor_nodes, state.key).unwrap_or(source_state_id);
            let source_scope_id = backend_scope_id_for_state(&state_scope_nodes, state.key);

            for transition in control_flow.transitions.span_or_empty(state.transitions) {
                append_transition_edges(
                    &mut diagram,
                    control_flow,
                    &state_nodes,
                    state.key,
                    source_anchor_id,
                    transition,
                );
            }

            for operation in control_flow.operations.span_or_empty(state.operations) {
                if let Some(target_key) = operation_call_target_key(control_flow, operation) {
                    if let Some(target_id) = state_node_id(&state_nodes, target_key) {
                        let target_scope_id =
                            backend_scope_id_for_state(&state_scope_nodes, target_key);
                        if source_scope_id == target_scope_id {
                            diagram.edge(source_anchor_id, target_id, "call");
                        } else if let Some(scope_target_id) = target_scope_id {
                            append_backend_external_call_node(
                                &mut diagram,
                                state.key,
                                source_anchor_id,
                                operation,
                                scope_target_id,
                            );
                        }
                    }
                }
            }
        }
    }

    diagram.finish()
}

fn build_machine_instruction_diagram(
    plan: &MachineInstructionPlan,
    control_flow: &ControlFlowPlan,
) -> String {
    let mut diagram = PhaseDiagramBuilder::new("machine_instructions");
    let mut machine_nodes = Vec::new();
    let mut state_nodes = Vec::<(StateKey, String)>::new();
    let mut state_scope_nodes = Vec::<(StateKey, String)>::new();
    let mut terminal_anchor_nodes = Vec::<(StateKey, String)>::new();

    for (machine_index, (_, machine)) in control_flow.machines.iter().enumerate() {
        let states = unique_machine_states(control_flow, machine);
        let root_keys = backend_visual_root_keys(control_flow, &states);

        for root_key in &root_keys {
            let Some(root_state) = backend_state_by_key_in_slice(&states, *root_key) else {
                continue;
            };
            let machine_id = diagram.node(
                format!(
                    "machine_{}_{}",
                    machine.symbol.arena_index(),
                    root_key.state.arena_index()
                ),
                backend_machine_label(machine, root_state),
                "machine",
                machine_index + 1,
            );
            machine_nodes.push((machine.symbol, *root_key, machine_id));
        }

        for state in states.iter().copied() {
            let root_key = backend_root_key_for_state(control_flow, &states, &root_keys, state.key);
            let Some(machine_id) =
                backend_machine_id_for_root(&machine_nodes, machine.symbol, root_key)
            else {
                continue;
            };
            let function = plan
                .functions
                .storage_slice()
                .iter()
                .find(|function| function.source_key == state.key);
            let instructions = function
                .map(|function| plan.instructions.span_or_empty(function.instructions))
                .unwrap_or(&[]);
            let lines = numbered_lines(instructions, machine_instruction_line);

            let block_title = machine_block_title(instructions);
            let state_id = diagram.node(
                format!(
                    "state_{}_{}_{}",
                    state.key.machine.arena_index(),
                    state.key.state.arena_index(),
                    state.key.segment_index
                ),
                state_backend_label_from_parts(control_flow, state, &block_title, &lines),
                "state_block",
                machine_index + 1,
            );
            diagram.node_details(
                &state_id,
                state_backend_details_from_parts(control_flow, state, &block_title, &lines),
            );
            diagram.containment_edge(machine_id, &state_id);
            state_nodes.push((state.key, state_id.clone()));
            state_scope_nodes.push((state.key, machine_id.to_owned()));

            let chunks = machine_instruction_chunks(&lines);
            let mut parent_chunk_id = state_id.clone();
            let mut terminal_anchor_id = state_id.clone();
            for chunk in &chunks {
                let chunk_id = diagram.node(
                    format!(
                        "machine_chunk_{}_{}_{}_{}",
                        state.key.machine.arena_index(),
                        state.key.state.arena_index(),
                        state.key.segment_index,
                        chunk.index
                    ),
                    machine_chunk_label(chunk, &state_scoped_name(control_flow, state.key)),
                    "scoped_statement",
                    machine_index + 1,
                );
                diagram.node_details(
                    &chunk_id,
                    machine_chunk_details(chunk, &state_scoped_name(control_flow, state.key)),
                );
                diagram.containment_edge(&parent_chunk_id, &chunk_id);
                parent_chunk_id = chunk_id.clone();
                terminal_anchor_id = chunk_id;
            }
            terminal_anchor_nodes.push((state.key, terminal_anchor_id));
        }
    }

    for (_, machine) in control_flow.machines.iter() {
        for state in control_flow.states.span_or_empty(machine.states) {
            let Some(source_state_id) = state_node_id(&state_nodes, state.key) else {
                continue;
            };
            let source_anchor_id =
                state_node_id(&terminal_anchor_nodes, state.key).unwrap_or(source_state_id);
            let source_scope_id = backend_scope_id_for_state(&state_scope_nodes, state.key);

            for transition in control_flow.transitions.span_or_empty(state.transitions) {
                append_transition_edges(
                    &mut diagram,
                    control_flow,
                    &state_nodes,
                    state.key,
                    source_anchor_id,
                    transition,
                );
            }

            for operation in control_flow.operations.span_or_empty(state.operations) {
                if let Some(target_key) = operation_call_target_key(control_flow, operation) {
                    if let Some(target_id) = state_node_id(&state_nodes, target_key) {
                        let target_scope_id =
                            backend_scope_id_for_state(&state_scope_nodes, target_key);
                        if source_scope_id == target_scope_id {
                            diagram.edge(source_state_id, target_id, "call");
                        } else if let Some(scope_target_id) = target_scope_id {
                            append_backend_external_call_node(
                                &mut diagram,
                                state.key,
                                source_state_id,
                                operation,
                                scope_target_id,
                            );
                        }
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
const MACHINE_CHUNK_PREVIEW_LINES: usize = 3;

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

fn state_backend_label(
    control_flow: &ControlFlowPlan,
    state: &StateFlow,
    function: Option<&FunctionView>,
) -> String {
    match function {
        Some(function) => {
            state_backend_label_from_parts(control_flow, state, &function.title, &function.lines)
        }
        None => state_backend_label_from_parts(control_flow, state, "no lowered block", &[]),
    }
}

fn state_backend_label_from_parts(
    control_flow: &ControlFlowPlan,
    state: &StateFlow,
    title: &str,
    lines_src: &[String],
) -> String {
    let mut lines = vec![
        format!(
            "{} [{}]",
            state_scoped_name(control_flow, state.key),
            state.key.segment_index
        ),
        title.to_owned(),
        state_flow_summary(control_flow, state),
    ];

    if lines_src.is_empty() {
        lines.push("no instructions".to_owned());
    } else {
        lines.push(format!("instructions: {}", lines_src.len()));
    }

    lines.join("\n")
}

fn state_backend_details(
    control_flow: &ControlFlowPlan,
    state: &StateFlow,
    function: Option<&FunctionView>,
) -> String {
    match function {
        Some(function) => {
            state_backend_details_from_parts(control_flow, state, &function.title, &function.lines)
        }
        None => state_backend_details_from_parts(control_flow, state, "no lowered block", &[]),
    }
}

fn state_backend_details_from_parts(
    control_flow: &ControlFlowPlan,
    state: &StateFlow,
    title: &str,
    lines_src: &[String],
) -> String {
    let mut lines = vec![
        format!(
            "{} [{}]",
            state_scoped_name(control_flow, state.key),
            state.key.segment_index
        ),
        title.to_owned(),
        state_flow_summary(control_flow, state),
    ];

    let transition_summaries = backend_transition_summaries(control_flow, state);
    if !transition_summaries.is_empty() {
        lines.push("transitions".to_owned());
        lines.extend(transition_summaries);
    }

    let call_summaries = backend_call_summaries(control_flow, state);
    if !call_summaries.is_empty() {
        lines.push("calls".to_owned());
        lines.extend(call_summaries);
    }

    if !lines_src.is_empty() {
        lines.push(format!("instructions: {}", lines_src.len()));
        let statement_count = distinct_statement_count(lines_src);
        if statement_count > 0 {
            lines.push(format!("source statements: {statement_count}"));
        }
        lines.push(String::new());
        lines.extend(group_lines_by_statement(lines_src));
    } else {
        lines.push("no instructions".to_owned());
    }

    lines.join("\n")
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

fn backend_transition_summaries(control_flow: &ControlFlowPlan, state: &StateFlow) -> Vec<String> {
    let mut lines = Vec::new();

    for (index, transition) in control_flow
        .transitions
        .span_or_empty(state.transitions)
        .iter()
        .enumerate()
    {
        if let Some(summary) =
            backend_transition_target_summary(control_flow, &transition.target, Some(state.key))
        {
            lines.push(format!("{index}. target -> {summary}"));
        }
        if let Some(summary) = backend_transition_target_summary(
            control_flow,
            &transition.continuation,
            Some(state.key),
        ) {
            lines.push(format!("{index}. continue -> {summary}"));
        }
    }

    lines
}

fn backend_call_summaries(control_flow: &ControlFlowPlan, state: &StateFlow) -> Vec<String> {
    let mut lines = Vec::new();

    for operation in control_flow.operations.span_or_empty(state.operations) {
        if let Some(target_key) = operation_call_target_key(control_flow, operation) {
            lines.push(format!(
                "#{} -> {}",
                operation.statement_index,
                state_scoped_name(control_flow, target_key)
            ));
        }
    }

    lines
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

fn distinct_statement_count(lines: &[String]) -> usize {
    let mut last = None::<usize>;
    let mut count = 0usize;
    for line in lines {
        let Some(statement) = statement_index_from_line(line) else {
            continue;
        };
        if last != Some(statement) {
            count += 1;
            last = Some(statement);
        }
    }
    count
}

fn group_lines_by_statement(lines: &[String]) -> Vec<String> {
    let mut grouped = Vec::new();
    let mut current_statement = None::<usize>;
    for line in lines {
        let statement = statement_index_from_line(line);
        if statement != current_statement {
            if !grouped.is_empty() {
                grouped.push(String::new());
            }
            if let Some(statement) = statement {
                grouped.push(format!("statement {statement}"));
            }
            current_statement = statement;
        }
        grouped.push(line.clone());
    }
    grouped
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

fn backend_visual_root_keys(plan: &ControlFlowPlan, states: &[&StateFlow]) -> Vec<StateKey> {
    let mut incoming = Vec::new();

    for state in states {
        for transition in plan.transitions.span_or_empty(state.transitions) {
            for target in [&transition.target, &transition.continuation] {
                if let Some(target_key) = backend_transition_target_key_in_states(states, target) {
                    if target_key != state.key && !incoming.contains(&target_key) {
                        incoming.push(target_key);
                    }
                }
            }
        }
    }

    let mut roots = states
        .iter()
        .filter(|state| !incoming.contains(&state.key))
        .map(|state| state.key)
        .collect::<Vec<_>>();
    if roots.is_empty() {
        if let Some(first) = states.first() {
            roots.push(first.key);
        }
    }
    roots
}

fn backend_root_key_for_state(
    plan: &ControlFlowPlan,
    states: &[&StateFlow],
    root_keys: &[StateKey],
    state_key: StateKey,
) -> StateKey {
    if root_keys.contains(&state_key) {
        return state_key;
    }

    for root_key in root_keys {
        if backend_reaches_state(plan, states, *root_key, state_key) {
            return *root_key;
        }
    }

    root_keys.first().copied().unwrap_or(state_key)
}

fn backend_reaches_state(
    plan: &ControlFlowPlan,
    states: &[&StateFlow],
    root_key: StateKey,
    target_key: StateKey,
) -> bool {
    let mut stack = vec![root_key];
    let mut visited = Vec::new();

    while let Some(key) = stack.pop() {
        if key == target_key {
            return true;
        }
        if visited.contains(&key) {
            continue;
        }
        visited.push(key);

        let Some(state) = backend_state_by_key_in_slice(states, key) else {
            continue;
        };
        for transition in plan.transitions.span_or_empty(state.transitions) {
            for target in [&transition.target, &transition.continuation] {
                if let Some(next_key) = backend_transition_target_key_in_states(states, target) {
                    stack.push(next_key);
                }
            }
        }
    }

    false
}

fn backend_transition_target_key_in_states(
    states: &[&StateFlow],
    target: &PlannedTransitionTarget,
) -> Option<StateKey> {
    match target {
        PlannedTransitionTarget::State { key, .. } => {
            states.iter().any(|state| state.key == *key).then_some(*key)
        }
        PlannedTransitionTarget::Nested { state_symbol, .. } => states
            .iter()
            .find(|state| state.key.state == *state_symbol)
            .map(|state| state.key),
        PlannedTransitionTarget::None
        | PlannedTransitionTarget::SelfTarget
        | PlannedTransitionTarget::Terminal => None,
    }
}

fn backend_state_by_key_in_slice<'states>(
    states: &'states [&StateFlow],
    key: StateKey,
) -> Option<&'states StateFlow> {
    states.iter().copied().find(|state| state.key == key)
}

fn backend_machine_label(machine: &MachineFlow, root_state: &StateFlow) -> String {
    format!(
        "{}\nentry slice: {} [{}]",
        machine.name.as_str(),
        root_state.name.as_str(),
        root_state.key.segment_index
    )
}

fn backend_machine_id_for_root(
    machine_nodes: &[(SymbolHandle, StateKey, String)],
    symbol: SymbolHandle,
    root_key: StateKey,
) -> Option<&str> {
    machine_nodes
        .iter()
        .find(|(machine_symbol, candidate_root_key, _)| {
            *machine_symbol == symbol && *candidate_root_key == root_key
        })
        .map(|node| node.2.as_str())
}

fn backend_scope_id_for_state(
    state_scope_nodes: &[(StateKey, String)],
    key: StateKey,
) -> Option<&str> {
    state_scope_nodes
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, id)| id.as_str())
}

fn append_backend_external_call_node(
    diagram: &mut PhaseDiagramBuilder,
    source_key: StateKey,
    source_id: &str,
    operation: &Operation,
    scope_target_id: &str,
) {
    let call_id = diagram.scoped_node(
        format!(
            "external_call_{}_{}_{}_{}",
            source_key.machine.arena_index(),
            source_key.state.arena_index(),
            source_key.segment_index,
            operation.statement_index
        ),
        format!(
            "external call\n{}\n\ndouble-click to scope target",
            backend_operation_label(operation)
        ),
        "external_call",
        source_key.machine.arena_index() as usize,
        scope_target_id,
    );
    diagram.edge(source_id, &call_id, "call");
    diagram.containment_edge(source_id, &call_id);
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
            let kind = if target_key == source_key {
                "transition_target_loopback"
            } else {
                "transition_target"
            };
            diagram.edge(source_id, target_id, kind);
        }
    }

    if let Some(target_key) =
        transition_target_key(control_flow, &transition.continuation, Some(source_key))
    {
        if let Some(target_id) = state_node_id(state_nodes, target_key) {
            let kind = if target_key == source_key {
                "transition_continuation_loopback"
            } else {
                "transition_continuation"
            };
            diagram.edge(source_id, target_id, kind);
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

fn backend_transition_target_summary(
    control_flow: &ControlFlowPlan,
    target: &PlannedTransitionTarget,
    source_key: Option<StateKey>,
) -> Option<String> {
    match target {
        PlannedTransitionTarget::Terminal => Some("terminal".to_owned()),
        PlannedTransitionTarget::SelfTarget => {
            source_key.map(|key| state_scoped_name(control_flow, key))
        }
        PlannedTransitionTarget::State { key, .. } => Some(state_scoped_name(control_flow, *key)),
        PlannedTransitionTarget::Nested {
            receiver_symbol,
            state_symbol,
            ..
        } => control_flow
            .state_key_by_symbols(*receiver_symbol, *state_symbol)
            .map(|key| state_scoped_name(control_flow, key)),
        PlannedTransitionTarget::None => None,
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

fn backend_operation_label(operation: &Operation) -> String {
    match &operation.kind {
        OperationKind::Assignment => format!("#{} assignment", operation.statement_index),
        OperationKind::Call {
            has_receiver,
            receiver,
            target,
            ..
        } => {
            if *has_receiver {
                format!(
                    "#{} call {}.{}(...)",
                    operation.statement_index,
                    receiver.as_str(),
                    target.as_str()
                )
            } else {
                format!(
                    "#{} call {}(...)",
                    operation.statement_index,
                    target.as_str()
                )
            }
        }
        OperationKind::ConstantIntegerAssignment => {
            format!("#{} const-int assign", operation.statement_index)
        }
        OperationKind::Expression => format!("#{} expr", operation.statement_index),
        OperationKind::LocalData => format!("#{} local data", operation.statement_index),
        OperationKind::StaticAssignment => format!("#{} static assign", operation.statement_index),
    }
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
    let prefix = if machine_instruction_is_call(instruction) {
        "call"
    } else if machine_instruction_is_control(instruction) {
        "ctrl"
    } else {
        "data"
    };
    format!(
        "{prefix} {:?} <- {} #{}",
        instruction.kind,
        enum_variant_name(&instruction.source_kind),
        instruction.selected_instruction_index
    )
}

fn machine_block_title(instructions: &[MachineInstruction]) -> String {
    let call_count = instructions
        .iter()
        .filter(|instruction| machine_instruction_is_call(instruction))
        .count();
    let control_count = instructions
        .iter()
        .filter(|instruction| machine_instruction_is_control(instruction))
        .count();
    let terminator = instructions
        .last()
        .map(|instruction| format!("{:?}", instruction.kind))
        .unwrap_or_else(|| "none".to_owned());
    format!("machine block\ncontrol: {control_count} calls: {call_count}\nterminator: {terminator}")
}

#[derive(Clone, Debug)]
struct MachineChunk {
    index: usize,
    first_line_index: usize,
    last_line_index: usize,
    lines: Vec<String>,
    first_statement: Option<usize>,
    last_statement: Option<usize>,
    control_count: usize,
    call_count: usize,
    terminator: String,
}

#[derive(Clone, Debug)]
struct OperationChunk {
    index: usize,
    first_line_index: usize,
    last_line_index: usize,
    lines: Vec<String>,
    statement_count: usize,
    first_statement: Option<usize>,
    last_statement: Option<usize>,
    control_count: usize,
    call_count: usize,
    terminator: String,
}

fn operation_chunks(lines: &[String]) -> Vec<OperationChunk> {
    let mut chunks = Vec::new();
    let mut current_lines = Vec::new();
    let mut first_line_index = 0usize;
    let mut call_count = 0usize;
    let mut control_count = 0usize;
    let mut last_statement = None::<usize>;
    let mut first_statement = None::<usize>;
    let mut statement_count = 0usize;

    for (index, line) in lines.iter().enumerate() {
        if current_lines.is_empty() {
            first_line_index = index;
            call_count = 0;
            control_count = 0;
            last_statement = None;
            first_statement = None;
            statement_count = 0;
        }

        current_lines.push(line.clone());
        let line_kind = operation_line_kind(line);
        if matches!(line_kind, OperationLineKind::Call) {
            call_count += 1;
        }
        if matches!(
            line_kind,
            OperationLineKind::Control | OperationLineKind::Call
        ) {
            control_count += 1;
        }

        let statement = statement_index_from_line(line);
        if first_statement.is_none() {
            first_statement = statement;
        }
        if statement != last_statement {
            if statement.is_some() {
                statement_count += 1;
            }
            last_statement = statement;
        }

        if matches!(
            line_kind,
            OperationLineKind::Control | OperationLineKind::Call
        ) {
            chunks.push(OperationChunk {
                index: chunks.len(),
                first_line_index,
                last_line_index: index,
                lines: std::mem::take(&mut current_lines),
                statement_count,
                first_statement,
                last_statement: statement,
                control_count,
                call_count,
                terminator: operation_line_head(line),
            });
        }
    }

    if !current_lines.is_empty() {
        let last_line_index = lines.len().saturating_sub(1);
        chunks.push(OperationChunk {
            index: chunks.len(),
            first_line_index,
            last_line_index,
            lines: current_lines,
            statement_count,
            first_statement,
            last_statement,
            control_count,
            call_count,
            terminator: "fallthrough".to_owned(),
        });
    }

    chunks
}

fn operation_chunk_label(chunk: &OperationChunk, origin: &str) -> String {
    let mut lines = vec![
        origin.to_owned(),
        format!(
            "B{} [{}..{}] {}",
            chunk.index,
            chunk.first_line_index,
            chunk.last_line_index,
            chunk_statement_span_label(chunk.first_statement, chunk.last_statement)
        ),
        format!(
            "statements: {} control: {} calls: {}",
            chunk.statement_count, chunk.control_count, chunk.call_count
        ),
        format!("terminator: {}", chunk.terminator),
    ];
    lines.extend(machine_chunk_preview_lines(&chunk.lines));
    lines.join("\n")
}

fn operation_chunk_details(chunk: &OperationChunk, origin: &str) -> String {
    let mut lines = vec![
        format!(
            "B{} [{}..{}]",
            chunk.index, chunk.first_line_index, chunk.last_line_index
        ),
        format!(
            "statements: {} control: {} calls: {}",
            chunk.statement_count, chunk.control_count, chunk.call_count
        ),
        format!("terminator: {}", chunk.terminator),
        format!("origin: {origin}"),
        String::new(),
    ];
    lines.extend(group_lines_by_statement(&chunk.lines));
    lines.join("\n")
}

fn machine_instruction_chunks(lines: &[String]) -> Vec<MachineChunk> {
    let mut chunks = Vec::new();
    let mut current_lines = Vec::new();
    let mut first_line_index = 0usize;
    let mut control_count = 0usize;
    let mut call_count = 0usize;
    let mut last_terminator = "none".to_owned();
    let mut first_statement = None::<usize>;
    let mut last_statement = None::<usize>;

    for (index, line) in lines.iter().enumerate() {
        if current_lines.is_empty() {
            first_line_index = index;
            control_count = 0;
            call_count = 0;
            last_terminator = "none".to_owned();
            first_statement = None;
            last_statement = None;
        }

        current_lines.push(line.clone());
        let statement = statement_index_from_line(line);
        if first_statement.is_none() {
            first_statement = statement;
        }
        last_statement = statement.or(last_statement);

        let kind = machine_line_kind(line);
        if matches!(kind, MachineLineKind::Call) {
            call_count += 1;
        }
        if !matches!(kind, MachineLineKind::Data) {
            control_count += 1;
            last_terminator = machine_line_head(line);
            chunks.push(MachineChunk {
                index: chunks.len(),
                first_line_index,
                last_line_index: index,
                lines: std::mem::take(&mut current_lines),
                first_statement,
                last_statement,
                control_count,
                call_count,
                terminator: last_terminator.clone(),
            });
        }
    }

    if !current_lines.is_empty() {
        let last_line_index = lines.len().saturating_sub(1);
        chunks.push(MachineChunk {
            index: chunks.len(),
            first_line_index,
            last_line_index,
            lines: current_lines,
            first_statement,
            last_statement,
            control_count,
            call_count,
            terminator: last_terminator,
        });
    }

    chunks
}

fn machine_chunk_label(chunk: &MachineChunk, origin: &str) -> String {
    let mut lines = vec![
        origin.to_owned(),
        format!(
            "B{} [{}..{}] {}",
            chunk.index,
            chunk.first_line_index,
            chunk.last_line_index,
            chunk_statement_span_label(chunk.first_statement, chunk.last_statement)
        ),
        format!(
            "control: {} calls: {}",
            chunk.control_count, chunk.call_count
        ),
        format!("terminator: {}", chunk.terminator),
    ];
    lines.extend(machine_chunk_preview_lines(&chunk.lines));
    lines.join("\n")
}

fn machine_chunk_details(chunk: &MachineChunk, origin: &str) -> String {
    let mut lines = vec![
        format!(
            "B{} [{}..{}]",
            chunk.index, chunk.first_line_index, chunk.last_line_index
        ),
        format!(
            "control: {} calls: {}",
            chunk.control_count, chunk.call_count
        ),
        format!("terminator: {}", chunk.terminator),
        format!("origin: {origin}"),
        String::new(),
    ];
    lines.extend(chunk.lines.iter().cloned());
    lines.join("\n")
}

fn machine_chunk_preview_lines(lines: &[String]) -> Vec<String> {
    let preview_count = lines.len().min(MACHINE_CHUNK_PREVIEW_LINES);
    let mut preview = lines
        .iter()
        .take(preview_count)
        .cloned()
        .collect::<Vec<_>>();
    if lines.len() > MACHINE_CHUNK_PREVIEW_LINES {
        preview.push(format!(
            "... {} more lines in details",
            lines.len() - MACHINE_CHUNK_PREVIEW_LINES
        ));
    }
    preview
}

fn chunk_statement_span_label(first: Option<usize>, last: Option<usize>) -> String {
    match (first, last) {
        (Some(first), Some(last)) if first == last => format!("stmt {first}"),
        (Some(first), Some(last)) => format!("stmt {first}..{last}"),
        (Some(first), None) => format!("stmt {first}"),
        _ => "stmt ?".to_owned(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MachineLineKind {
    Data,
    Call,
    Control,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OperationLineKind {
    Data,
    Call,
    Control,
}

fn machine_line_kind(line: &str) -> MachineLineKind {
    if line.contains(" call ") || line.contains(" call HostCallSequence ") {
        MachineLineKind::Call
    } else if line.contains(" ctrl ") || line.contains(" Dispatch") || line.contains(" Return") {
        MachineLineKind::Control
    } else {
        MachineLineKind::Data
    }
}

fn operation_line_kind(line: &str) -> OperationLineKind {
    let head = operation_line_head(line);
    if matches!(head.as_str(), "HostOperation" | "ReadRuntimeTextLine") {
        OperationLineKind::Call
    } else if head.starts_with("EnterDispatch")
        || head.starts_with("EvaluateDispatchGuard")
        || head.starts_with("SetDispatchState")
        || head.starts_with("LeaveDispatch")
        || head.starts_with("TerminateDispatch")
        || head.starts_with("LeaveFunction")
        || head.starts_with("CompareRuntime")
    {
        OperationLineKind::Control
    } else {
        OperationLineKind::Data
    }
}

fn operation_line_head(line: &str) -> String {
    let without_index = line.split_once(' ').map(|(_, rest)| rest).unwrap_or(line);
    without_index
        .split(" @ statement ")
        .next()
        .unwrap_or(without_index)
        .to_owned()
}

fn statement_index_from_line(line: &str) -> Option<usize> {
    let (_, suffix) = line.rsplit_once("@ statement ")?;
    suffix.trim().parse().ok()
}

fn machine_line_head(line: &str) -> String {
    let after_prefix = line.split_once(' ').map(|(_, rest)| rest).unwrap_or(line);
    after_prefix
        .split(" <- ")
        .next()
        .unwrap_or(after_prefix)
        .to_owned()
}

fn machine_instruction_is_call(instruction: &MachineInstruction) -> bool {
    matches!(
        instruction.kind,
        omega_machine_instructions::MachineInstructionKind::HostCallSequence
    )
}

fn machine_instruction_is_control(instruction: &MachineInstruction) -> bool {
    matches!(
        instruction.kind,
        omega_machine_instructions::MachineInstructionKind::DispatchLoopEnter
            | omega_machine_instructions::MachineInstructionKind::DispatchCaseEnter
            | omega_machine_instructions::MachineInstructionKind::DispatchGuardCompareStatic
            | omega_machine_instructions::MachineInstructionKind::DispatchStateWrite
            | omega_machine_instructions::MachineInstructionKind::DispatchTerminate
            | omega_machine_instructions::MachineInstructionKind::DispatchCaseLeave
            | omega_machine_instructions::MachineInstructionKind::HostCallSequence
            | omega_machine_instructions::MachineInstructionKind::Return
    )
}

fn enum_variant_name(value: &impl Debug) -> String {
    let debug = format!("{value:?}");
    let end = debug.find([' ', '{', '(']).unwrap_or(debug.len());
    debug[..end].to_owned()
}
