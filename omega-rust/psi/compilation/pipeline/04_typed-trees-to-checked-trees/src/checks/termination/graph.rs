use typed_trees::statement::TransitionTargetNode;

/// A transition edge between two states of a machine, identified by their
/// indices into `program.machine_states(machine)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct StateEdge {
    pub(super) from: usize,
    pub(super) to: usize,
}

pub(super) fn machine_has_cycle(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> bool {
    let adjacency = machine_adjacency(program, machine);

    adjacency
        .iter()
        .enumerate()
        .any(|(start, edges)| edges.contains(&start) || reaches_self(start, &adjacency))
}

/// An accepted proof entry call is a mathematical invocation whose exact
/// source decrease was already judged. Its progress still belongs to call
/// closure; it is not a second runtime state-cycle ranking obligation. Only
/// retained exact occurrences are removed, so other state cycles remain due.
pub(super) fn machine_has_cycle_after_proof_calls(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    self_call_descents: &[validation::ValidatedSelfCallDescent],
) -> bool {
    let adjacency = program
        .machine_states(machine)
        .iter()
        .map(|state| outgoing_state_indices(program, machine, state.symbol, self_call_descents))
        .collect::<Vec<_>>();
    adjacency
        .iter()
        .enumerate()
        .any(|(start, edges)| edges.contains(&start) || reaches_self(start, &adjacency))
}

/// Build the per-state adjacency list of the machine's transition graph. The
/// index of each entry corresponds to the state's position in
/// `program.machine_states(machine)`.
pub(crate) fn machine_adjacency(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Vec<Vec<usize>> {
    let states = program.machine_states(machine);
    states
        .iter()
        .map(|state| outgoing_state_indices(program, machine, state.symbol, &[]))
        .collect()
}

/// Compute the strongly connected components of the machine transition graph
/// using an iterative Tarjan traversal. Each returned component is a list of
/// state indices; components are returned in reverse topological order, which
/// is irrelevant to callers that only care about cycle membership.
pub(crate) fn strongly_connected_components(adjacency: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let count = adjacency.len();
    let mut index_counter = 0usize;
    let mut indices = vec![usize::MAX; count];
    let mut low_links = vec![0usize; count];
    let mut on_stack = vec![false; count];
    let mut stack: Vec<usize> = Vec::new();
    let mut components: Vec<Vec<usize>> = Vec::new();

    // Iterative DFS frame: the node being explored and the next neighbour to
    // visit within its adjacency list.
    for root in 0..count {
        if indices[root] != usize::MAX {
            continue;
        }

        let mut work: Vec<(usize, usize)> = vec![(root, 0)];
        while let Some((node, next_neighbour)) = work.pop() {
            if next_neighbour == 0 {
                indices[node] = index_counter;
                low_links[node] = index_counter;
                index_counter += 1;
                stack.push(node);
                on_stack[node] = true;
            }

            let neighbours = &adjacency[node];
            if next_neighbour < neighbours.len() {
                let neighbour = neighbours[next_neighbour];
                // Resume the parent after this neighbour is handled.
                work.push((node, next_neighbour + 1));
                if indices[neighbour] == usize::MAX {
                    work.push((neighbour, 0));
                } else if on_stack[neighbour] {
                    low_links[node] = low_links[node].min(indices[neighbour]);
                }
                continue;
            }

            // All neighbours explored: fold child low-links into the parent.
            for &neighbour in neighbours {
                if on_stack[neighbour] {
                    low_links[node] = low_links[node].min(low_links[neighbour]);
                }
            }

            if low_links[node] == indices[node] {
                let mut component = Vec::new();
                while let Some(member) = stack.pop() {
                    on_stack[member] = false;
                    component.push(member);
                    if member == node {
                        break;
                    }
                }
                components.push(component);
            }
        }
    }

    components
}

/// True when the component forms a cycle: either more than one state, or a
/// single state with a transition back to itself.
pub(crate) fn component_is_cyclic(adjacency: &[Vec<usize>], component: &[usize]) -> bool {
    if component.len() > 1 {
        return true;
    }
    component
        .first()
        .is_some_and(|&node| adjacency[node].contains(&node))
}

/// All transition edges whose source and target both belong to `component`.
/// These are exactly the edges that participate in the cycle and across which
/// a measure must strictly decrease.
pub(super) fn cyclic_edges(adjacency: &[Vec<usize>], component: &[usize]) -> Vec<StateEdge> {
    let mut edges = Vec::new();
    for &from in component {
        for &to in &adjacency[from] {
            if component.contains(&to) {
                edges.push(StateEdge { from, to });
            }
        }
    }
    edges
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
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state_symbol: symbols::SymbolHandle,
    self_call_descents: &[validation::ValidatedSelfCallDescent],
) -> Vec<usize> {
    let states = program.machine_states(machine);
    let Some(state) = typed_trees::state::state_by_symbol(states, state_symbol) else {
        return Vec::new();
    };
    let mut outgoing = Vec::new();

    for (statement_index, statement) in program
        .statement_table
        .indexed_statements(state.statement_nodes)
    {
        let targets = match statement {
            typed_trees::statement::StatementNode::Transition(transition) => {
                transition.targets().collect::<Vec<_>>()
            }
            typed_trees::statement::StatementNode::Expression(_) => vec![Default::default()],
            _ => continue,
        };
        for target in targets {
            let accepted_entry_call = self_call_descents.iter().any(|descent| {
                if descent.machine != machine.symbol
                    || descent.state != state.symbol
                    || descent.statement_index != statement_index
                    || program
                        .machine_states(machine)
                        .first()
                        .is_none_or(|entry| descent.entry != entry.symbol)
                {
                    return false;
                }
                let Some(site) = validation::find_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    descent.call_ordinal,
                ) else {
                    return false;
                };
                match site {
                    validation::CallSite::TransitionNamed { path, .. } => {
                        (path.symbol == machine.symbol || path.symbol == descent.entry)
                            && validation::transition_call_target(
                                program,
                                machine,
                                state,
                                statement_index,
                                descent.call_ordinal,
                            ) == Some(target)
                    }
                    validation::CallSite::Expression { expression, call } => {
                        let selected_expression = if target.is_valid() {
                            match program.statement_table.transition_target(target) {
                                TransitionTargetNode::Value(value) => Some(*value),
                                _ => None,
                            }
                        } else {
                            statement.as_expression().copied()
                        };
                        selected_expression == Some(expression)
                            && (call.target_symbol == machine.symbol
                                || call.target_symbol == descent.entry)
                    }
                    validation::CallSite::Statement(_) => false,
                }
            });
            if accepted_entry_call {
                continue;
            }
            if let Some(transfer) =
                source_state_transfer(program, machine, state, statement_index, target)
                && let Some(index) = states
                    .iter()
                    .position(|state| state.symbol == transfer.destination)
            {
                outgoing.push(index);
            }
        }
    }

    outgoing
}

/// Exact source control operands, before checked Flow retains their captures.
/// The original whole call stays available to all application/ranking/loan
/// judgments; this record adds no authority or claimed decrease.
/// A runtime receiver is evaluated before the positional arguments; Named
/// state transfers have no expression receiver. Physical reuse is separate.
pub(crate) struct SourceStateTransfer<'program> {
    pub(crate) destination: symbols::SymbolHandle,
    pub(crate) receiver: typed_trees::expression::ExpressionHandle,
    pub(crate) arguments: &'program [typed_trees::expression::ExpressionHandle],
}

pub(crate) fn source_state_transfer<'program>(
    program: &'program dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    target: typed_trees::statement::TransitionTargetHandle,
) -> Option<SourceStateTransfer<'program>> {
    let statement = program
        .statement_table
        .statement_at(state, statement_index)?;
    let expression = if target.is_valid() {
        if !statement
            .as_transition()?
            .targets()
            .any(|selected| selected == target)
        {
            return None;
        }
        match program.statement_table.transition_target(target) {
            TransitionTargetNode::Named {
                path, arguments, ..
            } => {
                let destination =
                    named_transition_target_state_index(program, machine, path.symbol)?;
                return Some(SourceStateTransfer {
                    destination: program.machine_states(machine).get(destination)?.symbol,
                    receiver: Default::default(),
                    arguments: program.statement_table.expression_handles(*arguments),
                });
            }
            TransitionTargetNode::Value(expression) => *expression,
            TransitionTargetNode::Terminal => return None,
        }
    } else {
        *statement.as_expression()?
    };
    let call = validation::source_tail_call(program, state, statement_index, expression)?;
    let entry = program.machine_states(machine).first()?;
    if call.target_symbol != machine.symbol && call.target_symbol != entry.symbol {
        return None;
    }
    Some(SourceStateTransfer {
        destination: program.machine_states(machine).first()?.symbol,
        receiver: call.receiver,
        arguments: program.expression_table.expression_handles(call.arguments),
    })
}

/// Resolve one named transition target to its state index inside `machine`.
/// A transition back to the machine's declared entry names the machine symbol,
/// while transitions to subordinate states name the state symbol. Ranking and
/// progress analysis must share this normalization so an entry back-edge is
/// not mistaken for a nested machine invocation.
pub(crate) fn named_transition_target_state_index(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    target_symbol: symbols::SymbolHandle,
) -> Option<usize> {
    ::validation::internal_state_index(program, machine, target_symbol)
}
