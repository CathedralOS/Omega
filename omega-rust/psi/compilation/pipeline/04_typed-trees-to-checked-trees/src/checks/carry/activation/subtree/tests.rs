use super::{compact_handle_ordinals, join_activation_carry};
use arena::{Handle, HandleSpan};
use checked_trees::{
    CarryFacts, ContainedMachineFieldFact, ContainedMachineTargetFact, MachineActivationCarryFact,
    MachineCarryTopologyFact,
};
use language_semantics::{CarryCpu, CarryHostThread, CarryPolicy};
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;

fn machine(position: u32) -> SymbolHandle {
    Handle::from_arena_index(position)
}

fn type_reference(position: u32) -> TypeReferenceHandle {
    Handle::from_arena_index(position)
}

fn direct_fact(machine: SymbolHandle, types: &[u32]) -> MachineActivationCarryFact {
    MachineActivationCarryFact {
        machine,
        effective: CarryPolicy::PERMISSIVE,
        analysis_complete: true,
        contributing_types: types.iter().copied().map(type_reference).collect(),
        unnamed_strict_values: 1,
    }
}

fn topology(carry: &mut CarryFacts, root: SymbolHandle, fields: &[&[SymbolHandle]]) {
    let mut field_span = HandleSpan::empty();
    for targets in fields {
        let target_span = carry.contained_targets.insert_many(
            targets
                .iter()
                .copied()
                .map(|machine| ContainedMachineTargetFact { machine }),
        );
        carry.contained_fields.append_to_span(
            &mut field_span,
            ContainedMachineFieldFact {
                targets: target_span,
                ..ContainedMachineFieldFact::default()
            },
        );
    }
    carry.machine_topologies.append(MachineCarryTopologyFact {
        machine: root,
        fields: field_span,
    });
}

// The former symbolic join is a deliberately simple test oracle. It consumes
// the public topology traversal, independently of compact ordinal preparation.
fn reference_join(
    carry: &CarryFacts,
    direct: &[MachineActivationCarryFact],
) -> Vec<MachineActivationCarryFact> {
    direct
        .iter()
        .map(|root| {
            let mut joined = MachineActivationCarryFact {
                machine: root.machine,
                effective: CarryPolicy::PERMISSIVE,
                analysis_complete: true,
                contributing_types: Vec::new(),
                unnamed_strict_values: 0,
            };
            for machine in carry.machine_subtree_symbols(root.machine) {
                let Some(fact) = direct.iter().find(|fact| fact.machine == machine) else {
                    joined.analysis_complete = false;
                    continue;
                };
                joined.effective = joined.effective.intersect(fact.effective);
                joined.analysis_complete &= fact.analysis_complete;
                joined.unnamed_strict_values = joined
                    .unnamed_strict_values
                    .saturating_add(fact.unnamed_strict_values);
                for type_reference in &fact.contributing_types {
                    if !joined.contributing_types.contains(type_reference) {
                        joined.contributing_types.push(*type_reference);
                    }
                }
            }
            joined
        })
        .collect()
}

#[test]
fn diamonds_cycles_and_repeated_fields_retain_breadth_first_contributions() {
    let mut carry = CarryFacts::default();
    // Root -> right, left; both -> leaf; leaf -> root. Include repeated
    // fields, a repeated target, and a self loop. Arena order is not BFS order.
    topology(&mut carry, machine(4), &[&[machine(1), machine(4)]]);
    topology(&mut carry, machine(2), &[&[machine(4)]]);
    topology(
        &mut carry,
        machine(1),
        &[&[machine(3), machine(2)], &[machine(3)]],
    );
    topology(&mut carry, machine(3), &[&[machine(4), machine(4)]]);
    let mut direct = vec![
        direct_fact(machine(1), &[9, 2]),
        direct_fact(machine(2), &[7, 2, 8]),
        direct_fact(machine(3), &[8, 9, 6]),
        direct_fact(machine(4), &[5, 7]),
        direct_fact(machine(5), &[99]),
    ];
    direct[1].effective.cpu = CarryCpu::Origin;
    direct[3].effective.host_thread = CarryHostThread::Origin;
    direct[3].analysis_complete = false;
    let joined = join_activation_carry(&carry, &direct);
    assert_eq!(joined, reference_join(&carry, &direct));
    assert_eq!(
        joined[0].contributing_types,
        [9, 2, 8, 6, 7, 5].map(type_reference)
    );
    assert_eq!(joined[0].unnamed_strict_values, 4);
    assert_eq!(joined[0].effective.cpu, CarryCpu::Origin);
    assert_eq!(joined[0].effective.host_thread, CarryHostThread::Origin);
    assert!(!joined[0].analysis_complete);
    assert_eq!(joined[4], direct[4]);
}

#[test]
fn missing_direct_facts_do_not_hide_strict_descendants() {
    let mut carry = CarryFacts::default();
    topology(&mut carry, machine(1), &[&[machine(2)]]);
    topology(&mut carry, machine(2), &[&[machine(3)]]);
    let mut direct = vec![direct_fact(machine(1), &[]), direct_fact(machine(3), &[3])];
    direct[1].effective = CarryPolicy::STRICT;
    direct[1].unnamed_strict_values = usize::MAX;
    let joined = join_activation_carry(&carry, &direct);
    assert_eq!(joined, reference_join(&carry, &direct));
    assert!(!joined[0].analysis_complete);
    assert_eq!(joined[0].effective, CarryPolicy::STRICT);
    assert_eq!(joined[0].unnamed_strict_values, usize::MAX);
    assert_eq!(joined[0].contributing_types, [type_reference(3)]);
}

#[test]
fn sparse_handles_and_generations_remain_distinct_without_sparse_storage() {
    let earlier = SymbolHandle::from_parts(u32::MAX, 7);
    let later = SymbolHandle::from_parts(u32::MAX, 8);
    let mut carry = CarryFacts::default();
    topology(&mut carry, earlier, &[&[later]]);
    let mut direct = vec![direct_fact(earlier, &[]), direct_fact(later, &[])];
    direct[0].contributing_types = vec![TypeReferenceHandle::from_parts(u32::MAX, 1)];
    direct[1].contributing_types = vec![TypeReferenceHandle::from_parts(u32::MAX, 2)];
    direct[1].effective = CarryPolicy::STRICT;
    let joined = join_activation_carry(&carry, &direct);
    assert_eq!(joined, reference_join(&carry, &direct));
    assert_eq!(joined[0].contributing_types.len(), 2);
    assert_eq!(joined[0].unnamed_strict_values, 2);
    assert_eq!(joined[1].contributing_types.len(), 1);
    assert_eq!(joined[1].unnamed_strict_values, 1);
    assert_eq!(
        compact_handle_ordinals([earlier, later, earlier].into_iter()),
        (vec![0, 1, 0], 2)
    );
}

#[test]
fn empty_input_and_first_exact_fact_semantics_are_preserved() {
    let mut carry = CarryFacts::default();
    assert!(join_activation_carry(&carry, &[]).is_empty());
    topology(&mut carry, machine(1), &[&[machine(2)]]);
    topology(&mut carry, machine(1), &[&[machine(3)]]);
    let direct = vec![
        direct_fact(machine(1), &[2, 2]),
        direct_fact(machine(2), &[8]),
        direct_fact(machine(1), &[9]),
    ];
    assert_eq!(
        join_activation_carry(&carry, &direct),
        reference_join(&carry, &direct)
    );
}

#[test]
fn growing_connected_graphs_match_the_symbolic_reference() {
    for count in [1, 16, 64, 128] {
        let mut carry = CarryFacts::default();
        let mut direct = Vec::new();
        for position in 1..=count {
            topology(
                &mut carry,
                machine(position),
                &[&[
                    machine(position % count + 1),
                    machine((position + count / 2) % count + 1),
                    machine(position),
                ]],
            );
            let mut fact = direct_fact(
                machine(position),
                &[
                    position * 6,
                    position * 6 + 1,
                    position * 6 + 2,
                    position * 6 + 3,
                    position * 6 + 4,
                    position * 6 + 5,
                    1,
                ],
            );
            if position % 7 == 0 {
                fact.effective = CarryPolicy::STRICT;
            }
            fact.analysis_complete = position % 11 != 0;
            direct.push(fact);
        }
        let prepared_start = std::time::Instant::now();
        let joined = join_activation_carry(&carry, &direct);
        let prepared_elapsed = prepared_start.elapsed();
        let reference_start = std::time::Instant::now();
        let reference = reference_join(&carry, &direct);
        eprintln!(
            "{count} machines: prepared {prepared_elapsed:?}, symbolic {:?}",
            reference_start.elapsed()
        );
        assert_eq!(joined, reference);
    }
}
