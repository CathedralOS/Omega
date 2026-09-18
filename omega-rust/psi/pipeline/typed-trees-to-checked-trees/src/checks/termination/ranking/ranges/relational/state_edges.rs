//! Entry-rooted parameter transfers establish exact state telescopes. Every
//! arrival then rechecks range membership; cyclic edges also owe strict descent.

use super::super::super::{graph, patterns};

use super::{preserved_entry_prefix, protected_input_paths};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;

pub(super) fn prove<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    range: ExpressionHandle,
    measure: validation::RankingRangeMeasure,
    frames: Option<&validation::CallFrameResolver<'program>>,
    premises: validation::RankingRangePremises,
    premise_inputs: &[SymbolHandle],
) -> bool {
    let states = program.machine_states(machine);
    let mut adjacency = graph::machine_adjacency(program, machine);
    // Topology needs one pair per destination; the occurrence reader below
    // still checks every distinct guarded transition to that destination.
    for targets in &mut adjacency {
        targets.sort_unstable();
        targets.dedup();
    }
    let entry_is_initial = !adjacency.iter().any(|targets| targets.contains(&0));
    // Every entry a computed arrival may carry. A single-subject view names
    // its one ranked carrier; a bounded distance ranks BOTH subjects, so a
    // computed actual mentioning copies of each claims the role its earliest
    // dependency names rather than staying role-less. A slice subject can
    // never carry a scalar slot — the destination's type validation rejects
    // the claim — while the slice slot itself selects its sole collection
    // dependency without any preference.
    let ranked_symbol = |subject: ExpressionHandle| {
        // A member chain names the formal it is rooted at: `record.field`
        // ranks that record formal's storage. Case projections and computed
        // receivers stay opaque — the edge judgment resolves the exact chain.
        let mut cursor = subject;
        while let ExpressionNode::Member(member) = program.expression_table.expression(cursor) {
            cursor = member.receiver;
        }
        match program.expression_table.expression(cursor) {
            ExpressionNode::Name(name)
                if name.symbol.is_valid() && name.head_symbol == name.symbol =>
            {
                Some(name.symbol)
            }
            _ => None,
        }
    };
    let mut preferred = Vec::new();
    let mut record_subject = SymbolHandle::default();
    match measure {
        validation::RankingRangeMeasure::Single(subject)
        | validation::RankingRangeMeasure::IncreasingTo { subject, .. } => {
            preferred.extend(ranked_symbol(subject));
        }
        validation::RankingRangeMeasure::Computed { subject, .. } => {
            preferred.extend(ranked_symbol(subject));
            // A scalar view over a member chain reads its root formal's
            // record; the telescope's fresh-literal claim needs that nominal
            // root to locate a dependency-free record arrival.
            if matches!(
                program.expression_table.expression(subject),
                ExpressionNode::Member(_)
            ) {
                record_subject = preferred.last().copied().unwrap_or_default();
            }
        }
        validation::RankingRangeMeasure::Field { subject, .. } => {
            preferred.extend(ranked_symbol(subject));
            record_subject = preferred.first().copied().unwrap_or_default();
        }
        validation::RankingRangeMeasure::Distance { lower, upper } => {
            preferred.extend(ranked_symbol(lower));
            preferred.extend(ranked_symbol(upper));
        }
        // A slice over projected storage ranks its root formal's record
        // slot: computed and literal arrivals of that record still claim
        // the role through the same carriers a field subject uses.
        validation::RankingRangeMeasure::SliceLength(subject) => {
            if matches!(
                program.expression_table.expression(subject),
                ExpressionNode::Member(_)
            ) {
                preferred.extend(ranked_symbol(subject));
                record_subject = preferred.last().copied().unwrap_or_default();
            }
        }
    }
    // The telescope is shared with the runtime call-component judgment: a
    // call issued from a subordinate state must read the same entry roles as
    // the member's own witness or the hypothesis would name a different value.
    // Discovery resolves contested claims against the same required set the
    // edge judgment enforces, so a duplicated required entry keeps every copy's
    // equality obligation while any other contested claim demotes to a bare
    // forward's unique carrier.
    let Some(required) =
        validation::ranking_range_required_symbols(program, machine, range, measure, premises)
    else {
        return false;
    };
    let Some(mappings) = validation::discover_state_entry_mappings_preferring(
        program,
        machine,
        &preferred,
        record_subject,
        &required,
    ) else {
        return false;
    };
    let components = graph::strongly_connected_components(&adjacency);
    for (source_position, source) in states.iter().enumerate() {
        // The same telescope that aliases entry roles onto this state's slots
        // decides which slots a prefix store must leave untouched.
        let protected = protected_input_paths(
            program,
            source,
            Some(&mappings[source_position]),
            premise_inputs,
        );
        for &target_position in &adjacency[source_position] {
            if source_position == 0 && target_position == 0 {
                continue;
            }
            let target = &states[target_position];
            let cyclic = components.iter().any(|component| {
                component.contains(&source_position)
                    && component.contains(&target_position)
                    && graph::component_is_cyclic(&adjacency, component)
            });
            for edge in patterns::edges_to_state(program, source, target.symbol) {
                let Some(prefix) = preserved_entry_prefix(
                    program,
                    machine,
                    source,
                    frames,
                    edge.statement_ordinal,
                    &protected,
                ) else {
                    return false;
                };
                let guards = edge
                    .guards
                    .iter()
                    .map(|guard| (guard.expression, guard.holds))
                    .collect::<Vec<_>>();
                if !validation::prove_ranking_range_transition(
                    program,
                    machine,
                    range,
                    measure,
                    if source_position == 0
                        && entry_is_initial
                        && matches!(premises, validation::RankingRangePremises::RankInvariant)
                    {
                        validation::RankingRangePremises::InitialEntry
                    } else {
                        premises
                    },
                    validation::RankingRangeState {
                        state: source,
                        entry_parameters: &mappings[source_position],
                    },
                    validation::RankingRangeState {
                        state: target,
                        entry_parameters: &mappings[target_position],
                    },
                    &guards,
                    &prefix,
                    edge.arguments,
                )
                .is_some_and(|proof| {
                    proof.membership_and_pinning && (!cyclic || proof.strictly_decreases)
                }) {
                    return false;
                }
            }
        }
    }
    true
}
