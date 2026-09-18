use super::{activation_crossing_validation_fixture, activation_target_fixture, topology_error};
use crate::task_plans::CheckedTrees;
use crate::task_plans::carry_crossings::{
    activation_carry_crossings, exact_activation_carry_subtree,
};
use crate::task_plans::start_selections::{
    TaskActivationTargetError, exact_task_activation_target,
};
use language_semantics::CarrySuspension;

#[test]
fn activation_target_retains_exact_machine_and_entry() {
    let (program, first_machine, first_entry, _, _) = activation_target_fixture();
    let (machine, entry) = exact_task_activation_target(&program, first_machine, first_entry)
        .expect("exact task target");

    assert_eq!(machine.symbol, first_machine);
    assert_eq!(entry.symbol, first_entry);
}

#[test]
fn activation_target_rejects_missing_entry() {
    let (program, first_machine, _, _, _) = activation_target_fixture();
    assert_eq!(
        exact_task_activation_target(&program, first_machine, symbols::SymbolHandle::invalid(),)
            .expect_err("missing entry must fail closed"),
        TaskActivationTargetError::MissingEntry
    );
}

#[test]
fn activation_target_rejects_duplicate_entry_within_owner() {
    let (_, first_machine, first_entry, _, _) = activation_target_fixture();
    let mut program = CheckedTrees::default();
    let mut machine = checked_trees::machine::Machine {
        symbol: first_machine,
        name: checked_trees::name::Identifier::generated("First::run"),
        ..Default::default()
    };
    for name in ["run", "duplicate"] {
        program.typed.push_machine_state(
            &mut machine,
            checked_trees::state::State {
                symbol: first_entry,
                name: checked_trees::name::Identifier::generated(name),
                ..Default::default()
            },
        );
    }
    program.typed.push_machine(machine);

    assert_eq!(
        exact_task_activation_target(&program, first_machine, first_entry)
            .expect_err("duplicate entry must fail closed"),
        TaskActivationTargetError::AmbiguousEntry
    );
}

#[test]
fn activation_target_rejects_duplicate_entry_across_owners() {
    let (mut program, first_machine, first_entry, second_machine, _) = activation_target_fixture();
    let second = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == second_machine)
        .expect("second machine")
        .clone();
    program.typed.machine_states_mut(&second)[0].symbol = first_entry;

    assert_eq!(
        exact_task_activation_target(&program, first_machine, first_entry)
            .expect_err("cross-owner duplicate entry must fail closed"),
        TaskActivationTargetError::AmbiguousEntry
    );
}

#[test]
fn activation_target_rejects_duplicate_owner_machine_identity() {
    let (mut program, first_machine, first_entry, second_machine, _) = activation_target_fixture();
    program.typed.machines.for_each_mut(|_, machine| {
        if machine.symbol == second_machine {
            machine.symbol = first_machine;
        }
    });

    assert_eq!(
        exact_task_activation_target(&program, first_machine, first_entry)
            .expect_err("duplicate owner machine identity must fail closed"),
        TaskActivationTargetError::AmbiguousMachine
    );
}

#[test]
fn activation_target_rejects_stored_machine_entry_drift() {
    let (program, first_machine, _, _, second_entry) = activation_target_fixture();
    assert_eq!(
        exact_task_activation_target(&program, first_machine, second_entry)
            .expect_err("stored machine/entry drift must fail closed"),
        TaskActivationTargetError::MachineMismatch
    );
}

#[test]
fn activation_crossings_include_contained_machine_crossings() {
    let (program, root, _, _) = activation_crossing_validation_fixture();
    let child_policy = program.facts.carry.suspension_crossings[0].effective;
    let crossings = activation_carry_crossings(&program, root).expect("exact crossing custody");

    assert!(
        crossings
            .subtree
            .iter()
            .all(|crossing| crossing.machine != root)
    );
    assert_eq!(crossings.subtree.len(), 2);
    assert_eq!(crossings.subtree[0].call_ordinal, 0);
    assert_eq!(crossings.subtree[1].call_ordinal, 1);
    assert!(
        crossings
            .subtree
            .iter()
            .all(|crossing| crossing.effective.suspension == CarrySuspension::Allowed)
    );
    assert_eq!(crossings.subtree[0].effective, child_policy);
}

#[test]
fn activation_topology_preserves_target_order_and_allows_cycles() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    let child = program.facts.carry.suspension_crossings[0].machine;
    let sibling = symbols::SymbolHandle::from_arena_index(20);
    program.typed.push_machine(checked_trees::machine::Machine {
        symbol: sibling,
        name: checked_trees::name::Identifier::generated("Sibling::run"),
        ..Default::default()
    });
    program
        .facts
        .carry
        .machine_topologies
        .insert(checked_trees::MachineCarryTopologyFact {
            machine: sibling,
            fields: arena::HandleSpan::empty(),
        });
    let existing_target = *program
        .facts
        .carry
        .contained_targets
        .iter()
        .next()
        .expect("child target")
        .1;
    let ordered_targets = program.facts.carry.contained_targets.insert_many([
        existing_target,
        checked_trees::ContainedMachineTargetFact { machine: sibling },
    ]);
    program
        .facts
        .carry
        .contained_fields
        .for_each_mut(|_, field| field.targets = ordered_targets);

    let type_reference = program
        .facts
        .carry
        .contained_fields
        .iter()
        .next()
        .expect("contained field")
        .1
        .type_reference;
    let cycle_targets = program
        .facts
        .carry
        .contained_targets
        .insert_many([checked_trees::ContainedMachineTargetFact { machine: root }]);
    let child_fields = program.facts.carry.contained_fields.insert_many([
        checked_trees::ContainedMachineFieldFact {
            field: symbols::SymbolHandle::from_arena_index(21),
            data: symbols::SymbolHandle::from_arena_index(22),
            type_reference,
            targets: cycle_targets,
        },
    ]);
    program
        .facts
        .carry
        .machine_topologies
        .for_each_mut(|_, topology| {
            if topology.machine == child {
                topology.fields = child_fields;
            }
        });

    assert_eq!(
        exact_activation_carry_subtree(&program, root).expect("exact ordered cyclic topology"),
        vec![root, child, sibling],
    );
}

#[test]
fn activation_topology_rejects_missing_reached_row() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    let child = program.facts.carry.suspension_crossings[0].machine;
    program
        .facts
        .carry
        .machine_topologies
        .for_each_mut(|_, topology| {
            if topology.machine == child {
                topology.machine = symbols::SymbolHandle::invalid();
            }
        });
    assert!(topology_error(&program, root).contains("one exact row per reached machine"));
}

#[test]
fn activation_topology_rejects_duplicate_reached_row() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    let duplicate = program
        .facts
        .carry
        .machine_topologies
        .iter()
        .find(|(_, topology)| topology.machine == root)
        .expect("root topology")
        .1
        .clone();
    program.facts.carry.machine_topologies.append(duplicate);
    assert!(topology_error(&program, root).contains("exactly one row per reached machine"));
}

#[test]
fn activation_topology_rejects_invalid_field_span() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    program
        .facts
        .carry
        .machine_topologies
        .for_each_mut(|_, topology| {
            if topology.machine == root {
                topology.fields = arena::HandleSpan::from_parts(arena::Handle::invalid(), 1);
            }
        });
    assert!(topology_error(&program, root).contains("exact valid field span"));
}

#[test]
fn activation_topology_rejects_empty_field_coordinate() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    program
        .facts
        .carry
        .contained_fields
        .for_each_mut(|_, field| field.field = symbols::SymbolHandle::invalid());
    assert!(topology_error(&program, root).contains("nonempty exact coordinates"));
}

#[test]
fn activation_topology_rejects_duplicate_field_coordinate() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    let field = program
        .facts
        .carry
        .contained_fields
        .iter()
        .next()
        .expect("contained field")
        .1
        .clone();
    let fields = program
        .facts
        .carry
        .contained_fields
        .insert_many([field.clone(), field]);
    program
        .facts
        .carry
        .machine_topologies
        .for_each_mut(|_, topology| {
            if topology.machine == root {
                topology.fields = fields;
            }
        });
    assert!(topology_error(&program, root).contains("unique within their machine"));
}

#[test]
fn activation_topology_rejects_invalid_target_span() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    program
        .facts
        .carry
        .contained_fields
        .for_each_mut(|_, field| {
            field.targets = arena::HandleSpan::from_parts(arena::Handle::invalid(), 1);
        });
    assert!(topology_error(&program, root).contains("exact valid target span"));
}

#[test]
fn activation_topology_rejects_empty_target_span() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    program
        .facts
        .carry
        .contained_fields
        .for_each_mut(|_, field| field.targets = arena::HandleSpan::empty());
    assert!(topology_error(&program, root).contains("at least one exact target"));
}

#[test]
fn activation_topology_rejects_duplicate_target() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    let target = *program
        .facts
        .carry
        .contained_targets
        .iter()
        .next()
        .expect("contained target")
        .1;
    let targets = program
        .facts
        .carry
        .contained_targets
        .insert_many([target, target]);
    program
        .facts
        .carry
        .contained_fields
        .for_each_mut(|_, field| field.targets = targets);
    assert!(topology_error(&program, root).contains("field targets must be unique"));
}

#[test]
fn activation_topology_rejects_missing_typed_target() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    program
        .facts
        .carry
        .contained_targets
        .for_each_mut(|_, target| target.machine = symbols::SymbolHandle::invalid());
    assert!(topology_error(&program, root).contains("target must name an exact typed machine"));
}
