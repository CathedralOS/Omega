//! The fused-state roster a composed control graph plans over.
//!
//! `fused_states` fuses every statically resolved terminal call into the
//! caller's graph. A terminal call runs in the current activation and
//! forwards its normal result, so the callee's states are transfers here,
//! not another activation. Mutually recursive transfers share one rank;
//! an acyclic target shares the same graph so its argument transfers and
//! temporary cleanup stay in the caller. The rest of this module's
//! machinery — signatures, entry claims, successors — reads this roster
//! through `FusedStates`, each state keeping its authored owner for
//! signatures and proof facts.
use super::super::StatementNode;

/// A composed graph's state roster: the dispatch machine's states followed by
/// every statically resolved terminal target. Each entry pairs the state with
/// its owning machine — signatures, entry claims, and every machine-scoped
/// fact answer through the owner, never the dispatch machine.
pub(in crate::execution::terminal_unit) type FusedStates<'program> = Vec<(
    &'program typed_trees::machine::Machine,
    &'program typed_trees::state::State,
)>;

/// Fuse every statically resolved terminal call into the caller's activation.
///
/// Discovery follows completing transition targets. A bodyless callable
/// contributes a contract and a call operation, never an executable state.
/// Each fused state retains its authored owner for signatures and proof facts.
/// Argument preparation and temporary cleanup of those states are this graph's
/// responsibility; they are not left on a separate call plan.
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
            // as successor lookup or the free form loses its owner.
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
    fused
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
