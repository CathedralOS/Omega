use omega_typed_trees::statement::{StatementNode, TransitionTargetNode};

pub(super) fn machine_has_cycle(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
) -> bool {
    let states = program.machine_states(machine);
    let adjacency = states
        .iter()
        .map(|state| outgoing_state_indices(program, machine, state.symbol))
        .collect::<Vec<_>>();

    adjacency
        .iter()
        .enumerate()
        .any(|(start, edges)| edges.contains(&start) || reaches_self(start, &adjacency))
}

fn reaches_self(start: usize, adjacency: &[Vec<usize>]) -> bool {
    let mut stack = adjacency[start].clone();
    let mut visited = vec![false; adjacency.len()];

    while let Some(index) = stack.pop() {
        if index == start {
            return true;
        }
        if visited[index] {
            continue;
        }
        visited[index] = true;
        stack.extend(adjacency[index].iter().copied());
    }

    false
}

fn outgoing_state_indices(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state_symbol: omega_core::symbols::SymbolHandle,
) -> Vec<usize> {
    let states = program.machine_states(machine);
    let Some(state) = states.iter().find(|state| state.symbol == state_symbol) else {
        return Vec::new();
    };
    let mut outgoing = Vec::new();

    for statement in program.statement_table.statements(state.statement_nodes) {
        let StatementNode::Transition(transition) = statement else {
            continue;
        };

        if let Some(index) = target_state_index(
            program,
            machine,
            program.statement_table.transition_target(transition.target),
        ) {
            outgoing.push(index);
        }
        if let Some(index) = target_state_index(
            program,
            machine,
            program
                .statement_table
                .transition_target(transition.continuation),
        ) {
            outgoing.push(index);
        }
    }

    outgoing
}

fn target_state_index(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    target: &TransitionTargetNode,
) -> Option<usize> {
    let target_symbol = match target {
        TransitionTargetNode::Named { path, .. } => path.symbol,
        TransitionTargetNode::SelfTarget => return None,
        TransitionTargetNode::Value(_) | TransitionTargetNode::Terminal => return None,
    };

    program
        .machine_states(machine)
        .iter()
        .position(|state| state.symbol == target_symbol)
}
