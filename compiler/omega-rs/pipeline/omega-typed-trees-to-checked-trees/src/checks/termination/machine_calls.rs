use std::collections::{BTreeMap, BTreeSet};

use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

use super::{graph, ranking};

#[derive(Clone)]
struct CallEdge {
    from: usize,
    to: usize,
    source_state: SymbolHandle,
    target_state: SymbolHandle,
    guard: ExpressionHandle,
    arguments: Vec<ExpressionHandle>,
    tail: bool,
}

pub(super) fn check_joint_machine_call_cycles(
    program: &omega_typed_trees::TypedTrees,
) -> Vec<Diagnostic> {
    let (adjacency, edges) = machine_call_graph(program);
    let machines = program.machines();
    let proof_only = omega_typed_trees::proof_only::classify(program);
    let mut diagnostics = Vec::new();

    for component in graph::strongly_connected_components(&adjacency)
        .into_iter()
        .filter(|component| graph::component_is_cyclic(&adjacency, component))
    {
        let names = component
            .iter()
            .map(|index| machines[*index].name.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        if !component
            .iter()
            .all(|index| machines[*index].ranking_witness.is_present())
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine call cycle `{names}` has no joint `terminates by` ranking witness"
            )));
            continue;
        }

        if !witness_shapes_match(program, &component) {
            diagnostics.push(Diagnostic::error(format!(
                "machine call cycle `{names}` does not use one same-shaped joint ranking witness"
            )));
            continue;
        }

        if let Some(machine) = component
            .iter()
            .map(|index| &machines[*index])
            .find(|machine| !ranking::machine_rank_range_proven(program, machine))
        {
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove authored rank range for `{}` in joint machine call cycle `{names}`",
                machine.name
            )));
            continue;
        }

        let members: BTreeSet<usize> = component.iter().copied().collect();
        let cycle_edges = edges
            .iter()
            .filter(|edge| members.contains(&edge.from) && members.contains(&edge.to))
            .collect::<Vec<_>>();
        let proof_component = component
            .iter()
            .all(|index| proof_only.is_proof_machine(program, &machines[*index]));
        if !proof_component && cycle_edges.iter().any(|edge| !edge.tail) {
            diagnostics.push(Diagnostic::error(format!(
                "runtime machine call cycle `{names}` contains a non-tail call; ranked runtime cycles must be tail-position calls"
            )));
            continue;
        }

        let mut nonstrict = Vec::new();
        let mut unproven = false;
        for edge in cycle_edges {
            let Some(source_state) = find_state(program, edge.source_state) else {
                unproven = true;
                break;
            };
            let Some(target_state) = find_state(program, edge.target_state) else {
                unproven = true;
                break;
            };
            let arguments = edge.arguments.as_slice();
            let class = if proof_component {
                classify_structural_proof_edge(
                    program,
                    &machines[edge.from],
                    &machines[edge.to],
                    arguments,
                )
            } else {
                ranking::classify_cross_machine_edge(
                    program,
                    &machines[edge.from],
                    source_state,
                    target_state,
                    edge.guard,
                    arguments,
                )
            };
            match class {
                ranking::EdgeClass::Strict => {}
                ranking::EdgeClass::NonIncreasing => nonstrict.push((edge.from, edge.to)),
                ranking::EdgeClass::Unknown => {
                    unproven = true;
                    break;
                }
            }
        }

        if unproven || !subgraph_is_acyclic(&component, &nonstrict) {
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove one joint `terminates by` ranking across machine call cycle `{names}`"
            )));
        }
    }

    diagnostics
}

fn classify_structural_proof_edge(
    program: &omega_typed_trees::TypedTrees,
    source: &omega_typed_trees::machine::Machine,
    target: &omega_typed_trees::machine::Machine,
    arguments: &[ExpressionHandle],
) -> ranking::EdgeClass {
    let source_subjects = program
        .expression_table
        .expression_handles(source.ranking_witness.subjects);
    let target_subjects = program
        .expression_table
        .expression_handles(target.ranking_witness.subjects);
    let ([source_subject], [target_subject]) = (source_subjects, target_subjects) else {
        return ranking::EdgeClass::Unknown;
    };
    let ExpressionNode::Name(source_path) = program.expression_table.expression(*source_subject)
    else {
        return ranking::EdgeClass::Unknown;
    };
    let ExpressionNode::Name(target_path) = program.expression_table.expression(*target_subject)
    else {
        return ranking::EdgeClass::Unknown;
    };
    let source_name = program
        .expression_table
        .name_path_members(source_path.members)
        .last()
        .map(|name| name.as_str());
    let target_name = program
        .expression_table
        .name_path_members(target_path.members)
        .last()
        .map(|name| name.as_str());
    let Some(target_state) = program.machine_states(target).first() else {
        return ranking::EdgeClass::Unknown;
    };
    let Some(argument_index) = program
        .state_parameters(target_state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .position(|parameter| {
            parameter.symbol == target_path.symbol
                || target_name.is_some_and(|name| parameter.name.as_str() == name)
        })
    else {
        return ranking::EdgeClass::Unknown;
    };
    let Some(argument) = arguments.get(argument_index).copied() else {
        return ranking::EdgeClass::Unknown;
    };

    if expression_is_name(program, argument, source_path.symbol, source_name) {
        ranking::EdgeClass::NonIncreasing
    } else if expression_is_strict_subterm(program, argument, source_path.symbol, source_name) {
        ranking::EdgeClass::Strict
    } else {
        ranking::EdgeClass::Unknown
    }
}

fn expression_is_strict_subterm(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    source_symbol: SymbolHandle,
    source_name: Option<&str>,
) -> bool {
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return false;
    };
    let mut root = member.receiver;
    loop {
        match program.expression_table.expression(root) {
            ExpressionNode::Member(member) => root = member.receiver,
            ExpressionNode::Name(_) => {
                return expression_is_name(program, root, source_symbol, source_name);
            }
            _ => return false,
        }
    }
}

fn expression_is_name(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
    name: Option<&str>,
) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    path.symbol == symbol
        || name.is_some_and(|name| {
            program
                .expression_table
                .name_path_members(path.members)
                .last()
                .is_some_and(|member| member.as_str() == name)
        })
}

fn witness_shapes_match(program: &omega_typed_trees::TypedTrees, component: &[usize]) -> bool {
    let Some(first) = component.first().map(|index| &program.machines()[*index]) else {
        return true;
    };
    let first_view = program.machine_decrease_order(first.ranking_witness.view);
    let first_subject_count = first.ranking_witness.subjects.count();
    let first_argument_count = first.ranking_witness.view_arguments.count();
    component.iter().skip(1).all(|index| {
        let witness = program.machines()[*index].ranking_witness;
        witness.subjects.count() == first_subject_count
            && witness.view_arguments.count() == first_argument_count
            && program.machine_decrease_order(witness.view) == first_view
    })
}

fn machine_call_graph(program: &omega_typed_trees::TypedTrees) -> (Vec<Vec<usize>>, Vec<CallEdge>) {
    let machines = program.machines();
    let mut state_owner = BTreeMap::new();
    for (machine_index, machine) in machines.iter().enumerate() {
        for state in program.machine_states(machine) {
            state_owner.insert(state.symbol.arena_index(), machine_index);
        }
    }

    let mut edges = Vec::new();
    for (machine_index, machine) in machines.iter().enumerate() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                collect_statement_edges(
                    program,
                    machine_index,
                    state.symbol,
                    statement,
                    &state_owner,
                    &mut edges,
                );
            }
        }
    }

    let mut adjacency = vec![Vec::new(); machines.len()];
    for edge in &edges {
        if edge.from != edge.to && !adjacency[edge.from].contains(&edge.to) {
            adjacency[edge.from].push(edge.to);
        }
    }
    for targets in &mut adjacency {
        targets.sort_unstable();
    }
    (adjacency, edges)
}

fn collect_statement_edges(
    program: &omega_typed_trees::TypedTrees,
    from: usize,
    source_state: SymbolHandle,
    statement: &StatementNode,
    state_owner: &BTreeMap<u32, usize>,
    edges: &mut Vec<CallEdge>,
) {
    match statement {
        StatementNode::Call(call) => {
            push_non_tail_edge(
                from,
                source_state,
                call.target_symbol,
                program
                    .statement_table
                    .expression_handles(call.arguments)
                    .to_vec(),
                state_owner,
                edges,
            );
            for argument in program.statement_table.expression_handles(call.arguments) {
                collect_expression_edges(
                    program,
                    from,
                    source_state,
                    *argument,
                    state_owner,
                    edges,
                );
            }
        }
        StatementNode::Assignment(assignment) => collect_expression_edges(
            program,
            from,
            source_state,
            assignment.value,
            state_owner,
            edges,
        ),
        StatementNode::Expression(expression) => {
            collect_expression_edges(program, from, source_state, *expression, state_owner, edges)
        }
        StatementNode::LocalData(local) => collect_expression_edges(
            program,
            from,
            source_state,
            local.initial_value,
            state_owner,
            edges,
        ),
        StatementNode::Transition(transition) => {
            let guard = match transition.guard {
                TransitionGuardNode::When(guard) => guard,
                TransitionGuardNode::Always => ExpressionHandle::invalid(),
            };
            if guard.is_valid() {
                collect_expression_edges(program, from, source_state, guard, state_owner, edges);
            }
            for (ordinal, target_handle) in [transition.target, transition.continuation]
                .into_iter()
                .enumerate()
            {
                if !target_handle.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target_handle) {
                    TransitionTargetNode::Named { path, arguments } => {
                        if let Some(&to) = state_owner.get(&path.symbol.arena_index())
                            && to != from
                        {
                            edges.push(CallEdge {
                                from,
                                to,
                                source_state,
                                target_state: path.symbol,
                                guard: if ordinal == 0 {
                                    guard
                                } else {
                                    ExpressionHandle::invalid()
                                },
                                arguments: program
                                    .statement_table
                                    .expression_handles(*arguments)
                                    .to_vec(),
                                tail: true,
                            });
                        }
                        for argument in program.statement_table.expression_handles(*arguments) {
                            collect_expression_edges(
                                program,
                                from,
                                source_state,
                                *argument,
                                state_owner,
                                edges,
                            );
                        }
                    }
                    TransitionTargetNode::Value(expression) => collect_expression_edges(
                        program,
                        from,
                        source_state,
                        *expression,
                        state_owner,
                        edges,
                    ),
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}

fn collect_expression_edges(
    program: &omega_typed_trees::TypedTrees,
    from: usize,
    source_state: SymbolHandle,
    expression: ExpressionHandle,
    state_owner: &BTreeMap<u32, usize>,
    edges: &mut Vec<CallEdge>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => {
            push_non_tail_edge(
                from,
                source_state,
                call.target_symbol,
                program
                    .expression_table
                    .expression_handles(call.arguments)
                    .to_vec(),
                state_owner,
                edges,
            );
            if call.receiver.is_valid() {
                collect_expression_edges(
                    program,
                    from,
                    source_state,
                    call.receiver,
                    state_owner,
                    edges,
                );
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_edges(
                    program,
                    from,
                    source_state,
                    *argument,
                    state_owner,
                    edges,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            for child in [binary.left, binary.right] {
                collect_expression_edges(program, from, source_state, child, state_owner, edges);
            }
        }
        ExpressionNode::Cast(cast) => {
            collect_expression_edges(program, from, source_state, cast.value, state_owner, edges)
        }
        ExpressionNode::Indexed(indexed) => {
            for child in [indexed.collection, indexed.index] {
                collect_expression_edges(program, from, source_state, child, state_owner, edges);
            }
        }
        ExpressionNode::Member(member) => collect_expression_edges(
            program,
            from,
            source_state,
            member.receiver,
            state_owner,
            edges,
        ),
        ExpressionNode::Mutable(value) => {
            collect_expression_edges(program, from, source_state, *value, state_owner, edges)
        }
        ExpressionNode::Range(range) => {
            for child in [range.start, range.end] {
                collect_expression_edges(program, from, source_state, child, state_owner, edges);
            }
        }
        ExpressionNode::Unary(unary) => collect_expression_edges(
            program,
            from,
            source_state,
            unary.operand,
            state_owner,
            edges,
        ),
        ExpressionNode::ArrayLiteral(items) => {
            for item in program.expression_table.expression_handles(*items) {
                collect_expression_edges(program, from, source_state, *item, state_owner, edges);
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                collect_expression_edges(
                    program,
                    from,
                    source_state,
                    field.value,
                    state_owner,
                    edges,
                );
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

fn push_non_tail_edge(
    from: usize,
    source_state: SymbolHandle,
    target_state: SymbolHandle,
    arguments: Vec<ExpressionHandle>,
    state_owner: &BTreeMap<u32, usize>,
    edges: &mut Vec<CallEdge>,
) {
    if let Some(&to) = state_owner.get(&target_state.arena_index())
        && to != from
    {
        edges.push(CallEdge {
            from,
            to,
            source_state,
            target_state,
            guard: ExpressionHandle::invalid(),
            arguments,
            tail: false,
        });
    }
}

fn find_state(
    program: &omega_typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<&omega_typed_trees::state::State> {
    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .find(|state| state.symbol == symbol)
}

fn subgraph_is_acyclic(component: &[usize], edges: &[(usize, usize)]) -> bool {
    fn visit(node: usize, edges: &[(usize, usize)], color: &mut BTreeMap<usize, u8>) -> bool {
        color.insert(node, 1);
        for &(from, to) in edges {
            if from != node {
                continue;
            }
            match color.get(&to).copied().unwrap_or(0) {
                1 => return false,
                0 if !visit(to, edges, color) => return false,
                _ => {}
            }
        }
        color.insert(node, 2);
        true
    }

    let mut color = BTreeMap::new();
    component
        .iter()
        .all(|node| color.get(node).copied().unwrap_or(0) != 0 || visit(*node, edges, &mut color))
}
