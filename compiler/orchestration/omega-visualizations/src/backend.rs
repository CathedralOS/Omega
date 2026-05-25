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

fn build_machine_instruction_diagram(
    plan: &MachineInstructionPlan,
    control_flow: &ControlFlowPlan,
) -> String {
    let mut diagram = PhaseDiagramBuilder::new("machine_instructions");
    let mut state_nodes = Vec::<(StateKey, String)>::new();
    let mut terminal_anchor_nodes = Vec::<(StateKey, String)>::new();

    for (machine_index, (_, machine)) in control_flow.machines.iter().enumerate() {
        let machine_id = diagram.node(
            format!("machine_{}", machine.symbol.arena_index()),
            machine_backend_label(machine),
            "machine",
            machine_index + 1,
        );

        for state in unique_machine_states(control_flow, machine) {
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
            diagram.containment_edge(&machine_id, &state_id);
            state_nodes.push((state.key, state_id.clone()));

            let chunks = machine_instruction_chunks(&lines);
            let mut previous_chunk_id = None::<String>;
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
                    machine_chunk_label(chunk),
                    "statement",
                    machine_index + 1,
                );
                diagram.node_details(&chunk_id, machine_chunk_details(chunk));
                diagram.containment_edge(&state_id, &chunk_id);
                if let Some(previous_chunk_id) = previous_chunk_id.as_deref() {
                    diagram.edge(previous_chunk_id, &chunk_id, "sequence");
                }
                previous_chunk_id = Some(chunk_id.clone());
                terminal_anchor_id = chunk_id;
            }
            terminal_anchor_nodes.push((state.key, terminal_anchor_id));
        }
    }

    for (_, machine) in control_flow.machines.iter() {
        for state in unique_machine_states(control_flow, machine) {
            let Some(source_state_id) = state_node_id(&state_nodes, state.key) else {
                continue;
            };
            let source_anchor_id =
                state_node_id(&terminal_anchor_nodes, state.key).unwrap_or(source_state_id);

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
                        diagram.edge(source_state_id, target_id, "call");
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

fn machine_backend_label(machine: &MachineFlow) -> String {
    machine.name.to_string()
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
        lines.extend(block_preview_lines(lines_src));
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

    if !lines_src.is_empty() {
        lines.push(format!("instructions: {}", lines_src.len()));
        lines.push(String::new());
        lines.extend(lines_src.iter().cloned());
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
    control_count: usize,
    call_count: usize,
    terminator: String,
}

fn machine_instruction_chunks(lines: &[String]) -> Vec<MachineChunk> {
    let mut chunks = Vec::new();
    let mut current_lines = Vec::new();
    let mut first_line_index = 0usize;
    let mut control_count = 0usize;
    let mut call_count = 0usize;
    let mut last_terminator = "none".to_owned();

    for (index, line) in lines.iter().enumerate() {
        if current_lines.is_empty() {
            first_line_index = index;
            control_count = 0;
            call_count = 0;
            last_terminator = "none".to_owned();
        }

        current_lines.push(line.clone());

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
            control_count,
            call_count,
            terminator: last_terminator,
        });
    }

    chunks
}

fn machine_chunk_label(chunk: &MachineChunk) -> String {
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
    ];
    lines.extend(machine_chunk_preview_lines(&chunk.lines));
    lines.join("\n")
}

fn machine_chunk_details(chunk: &MachineChunk) -> String {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MachineLineKind {
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
