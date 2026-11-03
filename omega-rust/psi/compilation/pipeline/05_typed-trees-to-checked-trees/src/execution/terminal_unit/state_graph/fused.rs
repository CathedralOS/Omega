//! The fused-state roster a composed control graph plans over.
//!
//! `fused_states` fuses the recursive component, not every transitively
//! reachable callee: mutually recursive tail transfers must stay in one
//! graph to retain their shared rank and constant-stack execution. The rest
//! of this module's machinery — signatures, entry claims, successors — reads
//! this roster through `FusedStates`, each state keeping its authored owner
//! for signatures and proof facts.
use super::super::StatementNode;

/// A composed graph's state roster: the dispatch machine's states followed by
/// the foreign machines in its recursive component. Each entry pairs the
/// state with its owning machine — signatures, entry claims, and every
/// machine-scoped fact answer through the owner, never the dispatch machine.
pub(in crate::execution::terminal_unit) type FusedStates<'program> = Vec<(
    &'program typed_trees::machine::Machine,
    &'program typed_trees::state::State,
)>;

/// Fuse the recursive component, not every transitively reachable callee.
/// Mutually recursive tail transfers must stay in one graph to retain their
/// shared rank and constant-stack execution. Acyclic calls keep the ordinary
/// call plan, including operand preparation and completion cleanup. Each
/// fused state retains its authored owner for signatures and proof facts.
pub(in crate::execution::terminal_unit) fn fused_states<'program>(
    program: &'program dyn ::validation::CheckingInput,
    machine: &'program typed_trees::machine::Machine,
) -> FusedStates<'program> {
    let mut fused: FusedStates<'program> = program
        .machine_states(machine)
        .iter()
        .map(|state| (machine, state))
        .collect();
    let mut cursor = 0;
    while let Some(&(_, state)) = fused.get(cursor) {
        cursor += 1;
        for target_symbol in completing_transition_targets(program, state) {
            if fused_target_index(program, &fused, target_symbol).is_some() {
                continue;
            }
            // Free transitions name a machine; attached transitions name its
            // entry state. Discovery must use the same exact head resolution
            // as successor lookup or the free form loses its recursive owner.
            let Some((target_owner, _)) = validation::find_machine_head(program, target_symbol)
            else {
                continue;
            };
            // A bodyless callable contributes a contract and a call
            // operation, never an executable state to this graph.
            if !target_owner.body_is_present {
                continue;
            }
            for owner_state in program.machine_states(target_owner) {
                if !fused
                    .iter()
                    .any(|(_, candidate)| candidate.symbol == owner_state.symbol)
                {
                    fused.push((target_owner, owner_state));
                }
            }
        }
    }
    // Discovery establishes forward reachability. Reverse reachability back
    // to the root keeps exactly its machine component. Include all states of
    // each retained owner, including acyclic finishing states.
    let mut owners = fused
        .iter()
        .map(|(owner, _)| owner.symbol)
        .collect::<Vec<_>>();
    owners.dedup(); // Discovery appends each owner's complete state roster once.
    if owners.len() <= 1 {
        return fused;
    }
    let mut predecessors = Vec::new();
    for (owner, state) in &fused {
        let Some(source_index) = owners.iter().position(|symbol| *symbol == owner.symbol) else {
            continue;
        };
        for target_symbol in completing_transition_targets(program, state) {
            let Some(target_index) = fused_target_index(program, &fused, target_symbol) else {
                continue;
            };
            let target_owner = fused[target_index].0.symbol;
            if let Some(target_index) = owners.iter().position(|symbol| *symbol == target_owner) {
                predecessors.push((target_index, source_index));
            }
        }
    }
    predecessors.sort_unstable();
    predecessors.dedup();
    let mut retained = vec![false; owners.len()];
    let mut ready = Vec::new();
    if let Some(root) = retained.first_mut() {
        *root = true;
        ready.push(0);
    }
    let mut cursor = 0;
    while let Some(&target) = ready.get(cursor) {
        cursor += 1;
        let first = predecessors.partition_point(|(candidate, _)| *candidate < target);
        for &(_, source) in predecessors[first..]
            .iter()
            .take_while(|(candidate, _)| *candidate == target)
        {
            if !retained[source] {
                retained[source] = true;
                ready.push(source);
            }
        }
    }
    // A terminal call runs in the current activation. When it moves an
    // activation claim, the callee must therefore join this graph even
    // though no cycle requires it: an ordinary call would carry the claim
    // into a separate machine, which is another activation. Calls that move
    // no such custody keep the ordinary call plan. Retention spreads from
    // every retained owner, so a claim moved again by the callee follows it.
    let mut cursor = 0;
    while let Some(&source) = ready.get(cursor) {
        cursor += 1;
        let source_states = fused
            .iter()
            .filter(|(owner, _)| owner.symbol == owners[source])
            .map(|(_, state)| *state);
        for state in source_states {
            for target_symbol in completing_transition_targets(program, state) {
                let Some(target_index) = fused_target_index(program, &fused, target_symbol) else {
                    continue;
                };
                let (target_owner, target_state) = fused[target_index];
                let Some(target) = owners
                    .iter()
                    .position(|symbol| *symbol == target_owner.symbol)
                else {
                    continue;
                };
                if !retained[target] && moves_activation_claim(program, target_state) {
                    retained[target] = true;
                    ready.push(target);
                }
            }
        }
    }
    fused.retain(|(owner, _)| {
        owners
            .iter()
            .position(|symbol| *symbol == owner.symbol)
            .is_some_and(|owner_index| retained[owner_index])
    });
    fused
}

/// Whether arriving at `entry` moves activation-scoped custody: one of its
/// owned (by-value) parameters carries an activation claim.
fn moves_activation_claim(
    program: &dyn ::validation::CheckingInput,
    entry: &typed_trees::state::State,
) -> bool {
    program.state_parameters(entry).iter().any(|parameter| {
        ::validation::type_carries_activation_claim(program, parameter.type_reference)
    })
}

/// Every complete transition invocation in authored order. Applied calls
/// retain their full source payload; exact resolution supplies the same
/// component edge as a named arrival without rewriting the typed tree.
fn completing_transition_targets<'a>(
    program: &'a dyn ::validation::CheckingInput,
    state: &'a typed_trees::state::State,
) -> impl Iterator<Item = symbols::SymbolHandle> + 'a {
    crate::flow::state_statements(program, state.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            StatementNode::Transition(transition) => Some(transition),
            _ => None,
        })
        .flat_map(|transition| transition.targets())
        .filter_map(|target| validation::completing_transition_transfer(program, target))
        .map(|transfer| transfer.target)
}

/// The fused position of a named transition target: a state symbol resolves
/// directly, while a machine head resolves to that owner's entry state —
/// the same normalization `activation_target_state_index` applies to
/// one machine's own roster.
pub(in crate::execution::terminal_unit) fn fused_target_index(
    program: &dyn ::validation::CheckingInput,
    fused: &FusedStates<'_>,
    target: symbols::SymbolHandle,
) -> Option<usize> {
    if let Some(index) = fused.iter().position(|(_, state)| state.symbol == target) {
        return Some(index);
    }
    let (_, entry) = validation::find_machine_head(program, target)?;
    fused
        .iter()
        .position(|(_, state)| state.symbol == entry.symbol)
}
