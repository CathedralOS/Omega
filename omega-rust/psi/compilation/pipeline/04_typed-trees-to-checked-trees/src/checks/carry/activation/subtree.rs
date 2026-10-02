//! Join direct envelopes over the checked containment graph, not child summaries:
//! diamonds and cycles must count each machine once. Prepare compact ordinals
//! once per invocation so overlapping root traversals do not search growing
//! symbol/type lists. Scratch is local; no cache or handle-lifetime assumptions
//! cross checking invocations. Published rosters retain root-first breadth-first
//! order, then the direct envelope's first-seen type order.

use arena::Handle;
use checked_trees::{CarryFacts, MachineActivationCarryFact};
use language_semantics::CarryPolicy;
use std::ops::Range;

#[cfg(test)]
mod tests;

pub(super) fn join_activation_carry(
    carry: &CarryFacts,
    direct: &[MachineActivationCarryFact],
) -> Vec<MachineActivationCarryFact> {
    // Retain missing-fact nodes too: an unresolved contained machine marks the
    // envelope incomplete, but its topology can still lead to known strict facts.
    let machine_handles =
        direct
            .iter()
            .map(|fact| fact.machine)
            .chain(carry.machine_topologies.iter().flat_map(|(_, topology)| {
                std::iter::once(topology.machine).chain(
                    carry
                        .contained_fields
                        .span_or_empty(topology.fields)
                        .iter()
                        .flat_map(|field| {
                            carry
                                .contained_targets_for_field(field)
                                .iter()
                                .map(|target| target.machine)
                        }),
                )
            }));
    let (machine_ordinals, machine_count) = compact_handle_ordinals(machine_handles);
    let (type_ordinals, type_count) = compact_handle_ordinals(
        direct
            .iter()
            .flat_map(|fact| fact.contributing_types.iter().copied()),
    );
    let mut facts = vec![None; machine_count];
    let mut type_ranges = Vec::with_capacity(direct.len());
    let mut type_start = 0;
    for (fact_position, fact) in direct.iter().enumerate() {
        // Match the first exact direct fact, as the former symbolic join did.
        facts[machine_ordinals[fact_position]].get_or_insert(fact_position);
        let type_end = type_start + fact.contributing_types.len();
        type_ranges.push(type_start..type_end);
        type_start = type_end;
    }

    let mut targets = vec![None::<Range<usize>>; machine_count];
    let mut topology_position = direct.len();
    for (_, topology) in carry.machine_topologies.iter() {
        let machine = machine_ordinals[topology_position];
        let target_start = topology_position + 1;
        let target_count = carry
            .contained_fields
            .span_or_empty(topology.fields)
            .iter()
            .map(|field| carry.contained_targets_for_field(field).len())
            .sum::<usize>();
        topology_position = target_start + target_count;
        targets[machine].get_or_insert(target_start..topology_position);
    }

    // Clear only visited entries between roots, not the entire domain. This
    // keeps disconnected machines cheap without generation/epoch rollover state.
    let mut visited = vec![false; machine_count];
    let mut seen_types = vec![false; type_count];
    let mut machines = Vec::new();
    let mut contributing_types = Vec::new();
    let mut joined_facts = Vec::with_capacity(direct.len());
    for (root_position, root) in direct.iter().enumerate() {
        let root_ordinal = machine_ordinals[root_position];
        machines.push(root_ordinal);
        visited[root_ordinal] = true;
        let mut joined = MachineActivationCarryFact {
            machine: root.machine,
            effective: CarryPolicy::PERMISSIVE,
            analysis_complete: true,
            contributing_types: Vec::new(),
            unnamed_strict_values: 0,
        };
        let mut cursor = 0;
        while cursor < machines.len() {
            let machine = machines[cursor];
            cursor += 1;
            if let Some(fact_position) = facts[machine] {
                let fact = &direct[fact_position];
                joined.effective = joined.effective.intersect(fact.effective);
                joined.analysis_complete &= fact.analysis_complete;
                joined.unnamed_strict_values = joined
                    .unnamed_strict_values
                    .saturating_add(fact.unnamed_strict_values);
                for (&type_reference, &type_ordinal) in fact
                    .contributing_types
                    .iter()
                    .zip(&type_ordinals[type_ranges[fact_position].clone()])
                {
                    if !seen_types[type_ordinal] {
                        seen_types[type_ordinal] = true;
                        contributing_types.push(type_ordinal);
                        joined.contributing_types.push(type_reference);
                    }
                }
            } else {
                joined.analysis_complete = false;
            }
            if let Some(targets) = &targets[machine] {
                for &target in &machine_ordinals[targets.clone()] {
                    if !visited[target] {
                        visited[target] = true;
                        machines.push(target);
                    }
                }
            }
        }
        for machine in machines.drain(..) {
            visited[machine] = false;
        }
        for type_ordinal in contributing_types.drain(..) {
            seen_types[type_ordinal] = false;
        }
        joined_facts.push(joined);
    }
    joined_facts
}

/// Assign one compact ordinal per full handle identity, returning ordinals in
/// input occurrence order. Sorting only preparation rows avoids searches inside
/// each root traversal and never changes the published traversal/type order.
fn compact_handle_ordinals<T>(handles: impl Iterator<Item = Handle<T>>) -> (Vec<usize>, usize) {
    let mut occurrences = handles.enumerate().collect::<Vec<_>>();
    occurrences.sort_unstable_by_key(|(_, handle)| (handle.arena_index(), handle.generation()));
    let mut ordinals = vec![0; occurrences.len()];
    let mut previous = None;
    let mut unique_count = 0;
    for (position, handle) in occurrences {
        if previous != Some(handle) {
            previous = Some(handle);
            unique_count += 1;
        }
        ordinals[position] = unique_count - 1;
    }
    (ordinals, unique_count)
}
