//! Cross-state flow primitives for default-domain validation.
//!
//! This module reconstructs the machine's transition graph and supplies the
//! must-meet for transported literal valuations. It does not schedule the
//! fixpoint, walk statements, or emit diagnostics.

use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::state::State;
use symbol_resolved_trees_to_typed_trees::typed_trees::statement::{
    StatementNode, TransitionTargetNode,
};

/// One place's transported field valuation (`None` value = known-unknown)
/// and its transported ACTIVE CASE (`None` = unknown or case-free). The case
/// rides the same must-meet as the literal fields: a constrained case's
/// `where` facts are only knowable when every predecessor agrees which case
/// is active.
pub(super) type PlaceValuation = (
    String,
    Vec<(String, Option<i128>)>,
    Option<symbols::SymbolHandle>,
);

/// Give semantic maps one publication order without changing their facts.
/// Missing fields and explicitly unknown values retain their existing meaning.
pub(super) fn canonicalize_valuations(valuations: &mut [PlaceValuation]) {
    valuations.sort_by(|left, right| left.0.cmp(&right.0));
    for (_, fields, _) in valuations {
        fields.sort_by(|left, right| left.0.cmp(&right.0));
    }
}

/// MUST meet of two exit valuations: a place survives only when present in
/// both; a field survives only when both sides agree on the SAME literal.
/// The active case survives only when both sides agree on the SAME case.
pub(super) fn meet_valuations(
    left: &[PlaceValuation],
    right: &[PlaceValuation],
) -> Vec<PlaceValuation> {
    let mut result = Vec::new();
    for (spelling, left_fields, left_case) in left {
        let Some((_, right_fields, right_case)) =
            right.iter().find(|(name, _, _)| name == spelling)
        else {
            continue;
        };
        let mut fields = Vec::new();
        for (field, left_value) in left_fields {
            if let Some((_, right_value)) = right_fields.iter().find(|(name, _)| name == field)
                && left_value == right_value
                && left_value.is_some()
            {
                fields.push((field.clone(), *left_value));
            }
        }
        let active_case = (left_case == right_case).then_some(*left_case).flatten();
        result.push((spelling.clone(), fields, active_case));
    }
    result
}

/// The machine's state-transition edges by state INDEX (Named targets
/// matched by simple state name; Value/Terminal/SelfTarget edges carry no
/// establishment transfer -- SelfTarget re-enters with the same entry set,
/// modeled as a self-edge).
pub(super) fn state_edges(program: &TypedTrees, states: &[State]) -> Vec<(usize, usize)> {
    let mut edges = Vec::new();
    for (from, state) in states.iter().enumerate() {
        for statement in program.statement_table.statements(state.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            for handle in [transition.target, transition.continuation] {
                if !handle.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(handle) {
                    // Resolve by SYMBOL first (the termination graph's proven
                    // rule), name as the fallback.
                    TransitionTargetNode::Named { path, .. } => {
                        let to = states
                            .iter()
                            .position(|candidate| candidate.symbol == path.symbol)
                            .or_else(|| {
                                program
                                    .expression_table
                                    .name_path_members(path.members)
                                    .last()
                                    .and_then(|target_name| {
                                        states
                                            .iter()
                                            .position(|candidate| candidate.name == *target_name)
                                    })
                            });
                        if let Some(to) = to {
                            edges.push((from, to));
                        }
                    }
                    TransitionTargetNode::SelfTarget => edges.push((from, from)),
                    _ => {}
                }
            }
        }
    }
    edges
}
