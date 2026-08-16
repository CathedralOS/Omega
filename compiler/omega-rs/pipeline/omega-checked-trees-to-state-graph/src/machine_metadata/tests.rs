use super::*;
use psi_arena::HandleSpan;
use psi_checked_trees::data::{DataDefinition, DataField, DataMember};
use psi_checked_trees::name::Identifier;
use psi_checked_trees::types::TypeReferenceNode;
use psi_checked_trees::{
    ContainedMachineFieldFact, ContainedMachineTargetFact, MachineCarryTopologyFact,
    MachineServiceReachRows, StateServiceReachRows,
};
use psi_language_semantics::{ServiceReachId, ServiceReachRowId, ServiceReachRowTable};
use psi_symbols::SymbolHandle;

#[test]
fn contained_topology_is_derived_only_from_fields_with_attached_machines() {
    let main_data_symbol = SymbolHandle::from_arena_index(1);
    let worker_data_symbol = SymbolHandle::from_arena_index(2);
    let scalar_data_symbol = SymbolHandle::from_arena_index(3);
    let main_machine_symbol = SymbolHandle::from_arena_index(4);
    let worker_machine_symbol = SymbolHandle::from_arena_index(5);
    let worker_field_symbol = SymbolHandle::from_arena_index(6);
    let scalar_field_symbol = SymbolHandle::from_arena_index(7);

    let mut program = CheckedTrees::default();
    let worker_type = program
        .typed
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: worker_data_symbol,
            name: Identifier::generated("Worker"),
        });
    let scalar_type = program
        .typed
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: scalar_data_symbol,
            name: Identifier::generated("Scalar"),
        });

    program.typed.push_data_definition(DataDefinition {
        symbol: worker_data_symbol,
        name: Identifier::generated("Worker"),
        ..Default::default()
    });
    program.typed.push_data_definition(DataDefinition {
        symbol: scalar_data_symbol,
        name: Identifier::generated("Scalar"),
        ..Default::default()
    });
    let mut main_data = DataDefinition {
        symbol: main_data_symbol,
        name: Identifier::generated("Main"),
        ..Default::default()
    };
    program.typed.push_data_member(
        &mut main_data,
        DataMember::Field(DataField {
            identity: None,
            symbol: worker_field_symbol,
            name: Identifier::generated("worker"),
            relevance: Default::default(),
            type_reference: worker_type,
        }),
    );
    program.typed.push_data_member(
        &mut main_data,
        DataMember::Field(DataField {
            identity: None,
            symbol: scalar_field_symbol,
            name: Identifier::generated("count"),
            relevance: Default::default(),
            type_reference: scalar_type,
        }),
    );
    program.typed.push_data_definition(main_data);

    program.typed.push_machine(Machine {
        symbol: worker_machine_symbol,
        name: Identifier::generated("Worker::run"),
        attached_data: Some(Identifier::generated("Worker")),
        ..Default::default()
    });
    let main_machine = Machine {
        symbol: main_machine_symbol,
        name: Identifier::generated("Main::main"),
        attached_data: Some(Identifier::generated("Main")),
        ..Default::default()
    };
    program.typed.push_machine(main_machine.clone());

    let targets = program
        .facts
        .carry
        .contained_targets
        .insert_many([ContainedMachineTargetFact {
            machine: worker_machine_symbol,
        }]);
    let fields = program
        .facts
        .carry
        .contained_fields
        .insert_many([ContainedMachineFieldFact {
            field: worker_field_symbol,
            data: worker_data_symbol,
            type_reference: worker_type,
            targets,
        }]);
    program
        .facts
        .carry
        .machine_topologies
        .insert(MachineCarryTopologyFact {
            machine: worker_machine_symbol,
            fields: HandleSpan::empty(),
        });
    program
        .facts
        .carry
        .machine_topologies
        .insert(MachineCarryTopologyFact {
            machine: main_machine_symbol,
            fields,
        });

    let mut graph = StateGraph::default();
    let contains = machine_contains(&mut graph, &program, &main_machine);
    let contained = graph.contained_machines.span_or_empty(contains);

    assert_eq!(contained.len(), 1);
    assert_eq!(contained[0].symbol, worker_field_symbol);
    assert_eq!(contained[0].name.as_str(), "worker");
    assert_eq!(contained[0].type_symbol, worker_machine_symbol);
    assert_eq!(contained[0].type_name.as_str(), "Worker");
}

struct ContainedTopologyFixture {
    program: CheckedTrees,
    root: Machine,
    root_data: SymbolHandle,
    leaf_data: SymbolHandle,
    field: SymbolHandle,
    leaf_type: psi_checked_trees::types::TypeReferenceHandle,
    first_target: SymbolHandle,
    second_target: SymbolHandle,
}

fn contained_topology_fixture(include_second_target: bool) -> ContainedTopologyFixture {
    let root_data = SymbolHandle::from_arena_index(71);
    let leaf_data = SymbolHandle::from_arena_index(72);
    let field = SymbolHandle::from_arena_index(73);
    let root_symbol = SymbolHandle::from_arena_index(74);
    let first_target = SymbolHandle::from_arena_index(75);
    let second_target = SymbolHandle::from_arena_index(76);
    let mut program = CheckedTrees::default();
    let leaf_type = program
        .typed
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: leaf_data,
            name: Identifier::generated("Leaf"),
        });
    program.typed.push_data_definition(DataDefinition {
        symbol: leaf_data,
        name: Identifier::generated("Leaf"),
        ..Default::default()
    });
    let mut root_definition = DataDefinition {
        symbol: root_data,
        name: Identifier::generated("Root"),
        ..Default::default()
    };
    program.typed.push_data_member(
        &mut root_definition,
        DataMember::Field(DataField {
            identity: None,
            symbol: field,
            name: Identifier::generated("leaf"),
            relevance: Default::default(),
            type_reference: leaf_type,
        }),
    );
    program.typed.push_data_definition(root_definition);
    program.typed.push_machine(Machine {
        symbol: first_target,
        name: Identifier::generated("Leaf::read"),
        attached_data: Some(Identifier::generated("Leaf")),
        ..Default::default()
    });
    if include_second_target {
        program.typed.push_machine(Machine {
            symbol: second_target,
            name: Identifier::generated("Leaf::write"),
            attached_data: Some(Identifier::generated("Leaf")),
            ..Default::default()
        });
    }
    let root = Machine {
        symbol: root_symbol,
        name: Identifier::generated("Root::run"),
        attached_data: Some(Identifier::generated("Root")),
        ..Default::default()
    };
    program.typed.push_machine(root.clone());

    let targets = program.facts.carry.contained_targets.insert_many(
        [first_target]
            .into_iter()
            .chain(include_second_target.then_some(second_target))
            .map(|machine| ContainedMachineTargetFact { machine }),
    );
    let fields = program
        .facts
        .carry
        .contained_fields
        .insert_many([ContainedMachineFieldFact {
            field,
            data: leaf_data,
            type_reference: leaf_type,
            targets,
        }]);
    program
        .facts
        .carry
        .machine_topologies
        .append(MachineCarryTopologyFact {
            machine: first_target,
            fields: HandleSpan::empty(),
        });
    if include_second_target {
        program
            .facts
            .carry
            .machine_topologies
            .append(MachineCarryTopologyFact {
                machine: second_target,
                fields: HandleSpan::empty(),
            });
    }
    program
        .facts
        .carry
        .machine_topologies
        .append(MachineCarryTopologyFact {
            machine: root_symbol,
            fields,
        });

    ContainedTopologyFixture {
        program,
        root,
        root_data,
        leaf_data,
        field,
        leaf_type,
        first_target,
        second_target,
    }
}

fn contained_topology_panic(program: &CheckedTrees, root: &Machine) -> String {
    metadata_panic(|| {
        let mut graph = StateGraph::default();
        machine_contains(&mut graph, program, root)
    })
}

fn root_contained_fields(
    program: &CheckedTrees,
    root: SymbolHandle,
) -> HandleSpan<ContainedMachineFieldFact> {
    program
        .facts
        .carry
        .topology_for_machine(root)
        .expect("root topology")
        .fields
}

#[test]
fn contained_topology_validates_all_targets_and_preserves_first_representative() {
    let mut fixture = contained_topology_fixture(true);
    fixture
        .program
        .facts
        .carry
        .machine_topologies
        .append(MachineCarryTopologyFact {
            machine: SymbolHandle::from_arena_index(77),
            fields: HandleSpan::empty(),
        });
    let mut graph = StateGraph::default();
    let contains = machine_contains(&mut graph, &fixture.program, &fixture.root);
    let contained = graph.contained_machines.span_or_empty(contains);

    assert_eq!(contained.len(), 1);
    assert_eq!(contained[0].type_symbol, fixture.first_target);
    assert_ne!(contained[0].type_symbol, fixture.second_target);
    assert_eq!(contained[0].type_name.as_str(), "Leaf");
}

#[test]
fn contained_topology_preserves_explicit_empty_row() {
    let mut fixture = contained_topology_fixture(false);
    fixture.program.facts.carry.machine_topologies = Default::default();
    fixture
        .program
        .facts
        .carry
        .machine_topologies
        .append(MachineCarryTopologyFact {
            machine: fixture.root.symbol,
            fields: HandleSpan::empty(),
        });
    let mut graph = StateGraph::default();
    let contains = machine_contains(&mut graph, &fixture.program, &fixture.root);

    assert!(contains.is_empty());
}

#[test]
fn contained_topology_rejects_missing_and_duplicate_machine_row() {
    let mut missing = contained_topology_fixture(false);
    missing.program.facts.carry.machine_topologies = Default::default();
    assert!(
        contained_topology_panic(&missing.program, &missing.root)
            .contains("topology row is missing")
    );

    let mut duplicate = contained_topology_fixture(false);
    duplicate
        .program
        .facts
        .carry
        .machine_topologies
        .append(MachineCarryTopologyFact {
            machine: duplicate.root.symbol,
            fields: HandleSpan::empty(),
        });
    assert!(
        contained_topology_panic(&duplicate.program, &duplicate.root)
            .contains("topology row is duplicated")
    );
}

#[test]
fn contained_topology_rejects_invalid_field_span_and_owner() {
    let mut invalid_span = contained_topology_fixture(false);
    invalid_span.program.facts.carry.machine_topologies = Default::default();
    invalid_span
        .program
        .facts
        .carry
        .machine_topologies
        .append(MachineCarryTopologyFact {
            machine: invalid_span.root.symbol,
            fields: HandleSpan::from_parts(
                psi_arena::Handle::<ContainedMachineFieldFact>::from_arena_index(999),
                1,
            ),
        });
    assert!(
        contained_topology_panic(&invalid_span.program, &invalid_span.root)
            .contains("field span is invalid")
    );

    let mut absent_owner = contained_topology_fixture(false);
    absent_owner
        .program
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.symbol == absent_owner.root.symbol)
        .expect("root machine")
        .attached_data = None;
    assert!(
        contained_topology_panic(&absent_owner.program, &absent_owner.root)
            .contains("no attached data owner")
    );

    let mut cross_owner = contained_topology_fixture(false);
    cross_owner
        .program
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.symbol == cross_owner.root.symbol)
        .expect("root machine")
        .attached_data = Some(Identifier::generated("Leaf"));
    assert!(
        contained_topology_panic(&cross_owner.program, &cross_owner.root)
            .contains("field belongs to another data owner")
    );
}

#[test]
fn contained_topology_rejects_field_tuple_and_identity_drift() {
    let mut tuple_drift = contained_topology_fixture(false);
    let fields = root_contained_fields(&tuple_drift.program, tuple_drift.root.symbol);
    tuple_drift
        .program
        .facts
        .carry
        .contained_fields
        .span_mut_or_empty(fields)[0]
        .data = tuple_drift.root_data;
    assert!(
        contained_topology_panic(&tuple_drift.program, &tuple_drift.root)
            .contains("data coordinate drifted")
    );

    let mut duplicate = contained_topology_fixture(false);
    duplicate.program.facts.carry.contained_fields = Default::default();
    let targets = duplicate
        .program
        .facts
        .carry
        .contained_targets
        .insert_many([ContainedMachineTargetFact {
            machine: duplicate.first_target,
        }]);
    let field = ContainedMachineFieldFact {
        field: duplicate.field,
        data: duplicate.leaf_data,
        type_reference: duplicate.leaf_type,
        targets,
    };
    let fields = duplicate
        .program
        .facts
        .carry
        .contained_fields
        .insert_many([field.clone(), field]);
    duplicate.program.facts.carry.machine_topologies = Default::default();
    duplicate
        .program
        .facts
        .carry
        .machine_topologies
        .append(MachineCarryTopologyFact {
            machine: duplicate.root.symbol,
            fields,
        });
    assert!(
        contained_topology_panic(&duplicate.program, &duplicate.root)
            .contains("field identity is empty or duplicated")
    );
}

#[test]
fn contained_topology_rejects_invalid_empty_and_duplicate_targets() {
    let mut empty = contained_topology_fixture(false);
    let fields = root_contained_fields(&empty.program, empty.root.symbol);
    empty
        .program
        .facts
        .carry
        .contained_fields
        .span_mut_or_empty(fields)[0]
        .targets = HandleSpan::empty();
    assert!(
        contained_topology_panic(&empty.program, &empty.root)
            .contains("target span is empty or invalid")
    );

    let mut invalid = contained_topology_fixture(false);
    let fields = root_contained_fields(&invalid.program, invalid.root.symbol);
    invalid
        .program
        .facts
        .carry
        .contained_fields
        .span_mut_or_empty(fields)[0]
        .targets = HandleSpan::from_parts(
        psi_arena::Handle::<ContainedMachineTargetFact>::from_arena_index(999),
        1,
    );
    assert!(
        contained_topology_panic(&invalid.program, &invalid.root)
            .contains("target span is empty or invalid")
    );

    let mut duplicate = contained_topology_fixture(false);
    let targets = duplicate
        .program
        .facts
        .carry
        .contained_targets
        .insert_many([
            ContainedMachineTargetFact {
                machine: duplicate.first_target,
            },
            ContainedMachineTargetFact {
                machine: duplicate.first_target,
            },
        ]);
    let fields = root_contained_fields(&duplicate.program, duplicate.root.symbol);
    duplicate
        .program
        .facts
        .carry
        .contained_fields
        .span_mut_or_empty(fields)[0]
        .targets = targets;
    assert!(
        contained_topology_panic(&duplicate.program, &duplicate.root)
            .contains("target identity is empty or duplicated")
    );
}

#[test]
fn contained_topology_rejects_missing_and_wrong_attached_target() {
    let mut missing = contained_topology_fixture(false);
    let targets =
        missing
            .program
            .facts
            .carry
            .contained_targets
            .insert_many([ContainedMachineTargetFact {
                machine: SymbolHandle::from_arena_index(999),
            }]);
    let fields = root_contained_fields(&missing.program, missing.root.symbol);
    missing
        .program
        .facts
        .carry
        .contained_fields
        .span_mut_or_empty(fields)[0]
        .targets = targets;
    assert!(
        contained_topology_panic(&missing.program, &missing.root)
            .contains("exact typed machine is missing")
    );

    let mut wrong_attached = contained_topology_fixture(false);
    let targets = wrong_attached
        .program
        .facts
        .carry
        .contained_targets
        .insert_many([ContainedMachineTargetFact {
            machine: wrong_attached.root.symbol,
        }]);
    let fields = root_contained_fields(&wrong_attached.program, wrong_attached.root.symbol);
    wrong_attached
        .program
        .facts
        .carry
        .contained_fields
        .span_mut_or_empty(fields)[0]
        .targets = targets;
    assert!(
        contained_topology_panic(&wrong_attached.program, &wrong_attached.root)
            .contains("attached to another data definition")
    );
}

fn push_operational_contract(
    program: &mut CheckedTrees,
    machine_symbol: SymbolHandle,
    checked_may_suspend: bool,
    checked_may_block: bool,
) {
    program
        .facts
        .suspensions
        .machines
        .push(psi_checked_trees::MachineSuspensionFact {
            machine: machine_symbol,
            plan: psi_language_semantics::SuspensionPlan {
                checked_may_suspend,
                ..Default::default()
            },
        });
    program
        .facts
        .blocking
        .machines
        .push(psi_checked_trees::MachineBlockingFact {
            machine: machine_symbol,
            plan: psi_language_semantics::BlockingPlan {
                checked_may_block,
                ..Default::default()
            },
        });
    program
        .facts
        .contract_plans
        .machines
        .push(psi_checked_trees::MachineContractPlan {
            machine: machine_symbol,
            closed_scalar_values: Default::default(),
            crash: Default::default(),
            fingerprint: 0,
        });
}

fn push_typed_machine_state(
    program: &mut CheckedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) {
    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Application::run"),
        ..Default::default()
    };
    program.typed.push_machine_state(
        &mut machine,
        psi_checked_trees::state::State {
            symbol: state_symbol,
            name: Identifier::generated("main"),
            ..Default::default()
        },
    );
    program.typed.push_machine(machine);
}

fn push_service_reach(
    program: &mut CheckedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    machine_row: ServiceReachRowId,
    state_row: ServiceReachRowId,
) {
    let states = program
        .facts
        .service_reaches
        .states
        .insert_many([StateServiceReachRows {
            state: state_symbol,
            inferred_direct: state_row,
            inferred_transitive: state_row,
            ..Default::default()
        }]);
    program.facts.service_reaches.machines.append_to_span(
        &mut program.facts.service_reaches.root_machines,
        MachineServiceReachRows {
            machine: machine_symbol,
            inferred_direct: machine_row,
            inferred_transitive: machine_row,
            states,
            ..Default::default()
        },
    );
}

fn push_operational_flow_state(
    program: &mut CheckedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    suspension: psi_language_semantics::SuspensionSummary,
    blocking: psi_language_semantics::BlockingSummary,
) {
    program
        .facts
        .flow
        .control
        .states
        .insert(psi_checked_trees::FlowStateFact {
            machine_symbol,
            state_symbol,
            suspension,
            blocking,
            ..Default::default()
        });
}

fn complete_metadata_program() -> (CheckedTrees, SymbolHandle, SymbolHandle) {
    let machine = SymbolHandle::from_arena_index(51);
    let state = SymbolHandle::from_arena_index(52);
    let mut program = CheckedTrees::default();
    push_typed_machine_state(&mut program, machine, state);
    push_operational_contract(&mut program, machine, false, false);
    push_operational_flow_state(
        &mut program,
        machine,
        state,
        Default::default(),
        Default::default(),
    );
    push_service_reach(
        &mut program,
        machine,
        state,
        ServiceReachRowTable::EMPTY_ROW,
        ServiceReachRowTable::EMPTY_ROW,
    );
    (program, machine, state)
}

fn metadata_panic<T: std::fmt::Debug>(action: impl FnOnce() -> T) -> String {
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(action))
        .expect_err("invalid metadata must fail closed");
    panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            panic
                .downcast_ref::<&str>()
                .map(|message| (*message).to_owned())
        })
        .expect("metadata invariant panic has a string diagnostic")
}

#[test]
fn graph_metadata_publishes_suspension_without_blocking() {
    let machine_symbol = SymbolHandle::from_arena_index(21);
    let state_symbol = SymbolHandle::from_arena_index(22);

    let mut program = CheckedTrees::default();
    push_typed_machine_state(&mut program, machine_symbol, state_symbol);
    push_operational_contract(&mut program, machine_symbol, true, false);
    push_operational_flow_state(
        &mut program,
        machine_symbol,
        state_symbol,
        psi_language_semantics::SuspensionSummary {
            direct_may_suspend: true,
            transitive_may_suspend: true,
        },
        Default::default(),
    );

    assert_eq!(
        machine_suspension_summary(&program, machine_symbol),
        psi_language_semantics::SuspensionSummary {
            direct_may_suspend: true,
            transitive_may_suspend: true,
        }
    );
    assert_eq!(
        machine_blocking_summary(&program, machine_symbol),
        psi_language_semantics::BlockingSummary::default()
    );
    assert_eq!(
        state_suspension_summary(&program, state_symbol),
        psi_language_semantics::SuspensionSummary {
            direct_may_suspend: true,
            transitive_may_suspend: true,
        }
    );
    assert_eq!(
        state_blocking_summary(&program, state_symbol),
        psi_language_semantics::BlockingSummary::default()
    );
}

#[test]
fn graph_metadata_publishes_blocking_without_suspension() {
    let machine_symbol = SymbolHandle::from_arena_index(31);
    let state_symbol = SymbolHandle::from_arena_index(32);

    let mut program = CheckedTrees::default();
    push_typed_machine_state(&mut program, machine_symbol, state_symbol);
    push_operational_contract(&mut program, machine_symbol, false, true);
    push_operational_flow_state(
        &mut program,
        machine_symbol,
        state_symbol,
        Default::default(),
        psi_language_semantics::BlockingSummary {
            direct_may_block: true,
            transitive_may_block: true,
        },
    );

    assert_eq!(
        machine_suspension_summary(&program, machine_symbol),
        psi_language_semantics::SuspensionSummary::default()
    );
    assert_eq!(
        machine_blocking_summary(&program, machine_symbol),
        psi_language_semantics::BlockingSummary {
            direct_may_block: true,
            transitive_may_block: true,
        }
    );
    assert_eq!(
        state_suspension_summary(&program, state_symbol),
        psi_language_semantics::SuspensionSummary::default()
    );
    assert_eq!(
        state_blocking_summary(&program, state_symbol),
        psi_language_semantics::BlockingSummary {
            direct_may_block: true,
            transitive_may_block: true,
        }
    );
}

#[test]
#[should_panic(expected = "exact typed machine is missing")]
fn graph_metadata_rejects_unknown_machine_coordinate() {
    let program = CheckedTrees::default();
    let unknown_machine = SymbolHandle::from_arena_index(41);
    let _ = machine_suspension_summary(&program, unknown_machine);
}

#[test]
#[should_panic(expected = "exact typed state is missing")]
fn graph_metadata_rejects_unknown_state_coordinate() {
    let program = CheckedTrees::default();
    let _ = state_service_reach(&program, SymbolHandle::from_arena_index(42));
}

#[test]
fn graph_metadata_rejects_duplicate_typed_state_owner() {
    let state = SymbolHandle::from_arena_index(43);
    let mut program = CheckedTrees::default();
    push_typed_machine_state(&mut program, SymbolHandle::from_arena_index(44), state);
    push_typed_machine_state(&mut program, SymbolHandle::from_arena_index(45), state);

    assert!(
        metadata_panic(|| state_suspension_summary(&program, state))
            .contains("exact typed state is duplicated")
    );
}

#[test]
fn graph_metadata_preserves_exact_empty_and_negative_axes() {
    let (mut program, machine, state) = complete_metadata_program();
    let unrelated = SymbolHandle::from_arena_index(53);
    program
        .facts
        .suspensions
        .machines
        .push(psi_checked_trees::MachineSuspensionFact {
            machine: unrelated,
            plan: psi_language_semantics::SuspensionPlan {
                checked_may_suspend: true,
                ..Default::default()
            },
        });
    program
        .facts
        .blocking
        .machines
        .push(psi_checked_trees::MachineBlockingFact {
            machine: unrelated,
            plan: psi_language_semantics::BlockingPlan {
                checked_may_block: true,
                ..Default::default()
            },
        });

    assert_eq!(
        machine_service_reach(&program, machine),
        psi_language_semantics::ServiceReachSummary {
            direct: ServiceReachRowTable::EMPTY_ROW,
            transitive: ServiceReachRowTable::EMPTY_ROW,
        }
    );
    assert_eq!(
        state_service_reach(&program, state),
        psi_language_semantics::ServiceReachSummary {
            direct: ServiceReachRowTable::EMPTY_ROW,
            transitive: ServiceReachRowTable::EMPTY_ROW,
        }
    );
    assert_eq!(
        machine_suspension_summary(&program, machine),
        psi_language_semantics::SuspensionSummary::default()
    );
    assert_eq!(
        machine_blocking_summary(&program, machine),
        psi_language_semantics::BlockingSummary::default()
    );
}

#[test]
fn graph_metadata_rejects_missing_and_duplicate_machine_axes() {
    let (mut missing_service, machine, _) = complete_metadata_program();
    missing_service.facts.service_reaches = Default::default();
    assert!(
        metadata_panic(|| machine_service_reach(&missing_service, machine))
            .contains("machine service-reach row is missing")
    );

    let (mut duplicate_service, machine, _) = complete_metadata_program();
    duplicate_service
        .facts
        .service_reaches
        .machines
        .append_to_span(
            &mut duplicate_service.facts.service_reaches.root_machines,
            MachineServiceReachRows {
                machine,
                inferred_direct: ServiceReachRowTable::EMPTY_ROW,
                inferred_transitive: ServiceReachRowTable::EMPTY_ROW,
                ..Default::default()
            },
        );
    assert!(
        metadata_panic(|| machine_service_reach(&duplicate_service, machine))
            .contains("machine service-reach row is duplicated")
    );

    let (mut missing_suspension, machine, _) = complete_metadata_program();
    missing_suspension.facts.suspensions.machines.clear();
    assert!(
        metadata_panic(|| machine_suspension_summary(&missing_suspension, machine))
            .contains("machine suspension row is missing")
    );

    let (mut duplicate_suspension, machine, _) = complete_metadata_program();
    let duplicate = duplicate_suspension.facts.suspensions.machines[0];
    duplicate_suspension
        .facts
        .suspensions
        .machines
        .push(duplicate);
    assert!(
        metadata_panic(|| machine_suspension_summary(&duplicate_suspension, machine))
            .contains("machine suspension row is duplicated")
    );

    let (mut missing_blocking, machine, _) = complete_metadata_program();
    missing_blocking.facts.blocking.machines.clear();
    assert!(
        metadata_panic(|| machine_blocking_summary(&missing_blocking, machine))
            .contains("machine blocking row is missing")
    );

    let (mut duplicate_blocking, machine, _) = complete_metadata_program();
    let duplicate = duplicate_blocking.facts.blocking.machines[0];
    duplicate_blocking.facts.blocking.machines.push(duplicate);
    assert!(
        metadata_panic(|| machine_blocking_summary(&duplicate_blocking, machine))
            .contains("machine blocking row is duplicated")
    );
}

#[test]
fn graph_metadata_rejects_invalid_and_unregistered_service_rows() {
    let (mut invalid, machine, state) = complete_metadata_program();
    invalid.facts.service_reaches = Default::default();
    push_service_reach(
        &mut invalid,
        machine,
        state,
        ServiceReachRowId(99),
        ServiceReachRowId(99),
    );
    assert!(
        metadata_panic(|| machine_service_reach(&invalid, machine))
            .contains("row identity is noncanonical")
    );

    let (mut unregistered, machine, state) = complete_metadata_program();
    unregistered.facts.service_reaches = Default::default();
    let row = unregistered
        .facts
        .service_reaches
        .rows
        .intern(vec![ServiceReachId(99)]);
    push_service_reach(&mut unregistered, machine, state, row, row);
    assert!(
        metadata_panic(|| state_service_reach(&unregistered, state))
            .contains("unregistered service")
    );
}

#[test]
fn graph_metadata_rejects_missing_duplicate_and_cross_machine_flow_state() {
    let (mut missing, _, state) = complete_metadata_program();
    missing.facts.flow.control.states = Default::default();
    assert!(
        metadata_panic(|| state_suspension_summary(&missing, state))
            .contains("FlowState fact is missing")
    );

    let (mut duplicate, machine, state) = complete_metadata_program();
    push_operational_flow_state(
        &mut duplicate,
        machine,
        state,
        Default::default(),
        Default::default(),
    );
    assert!(
        metadata_panic(|| state_blocking_summary(&duplicate, state))
            .contains("FlowState fact is duplicated")
    );

    let (mut cross_machine, _, state) = complete_metadata_program();
    cross_machine.facts.flow.control.states = Default::default();
    push_operational_flow_state(
        &mut cross_machine,
        SymbolHandle::from_arena_index(54),
        state,
        Default::default(),
        Default::default(),
    );
    assert!(
        metadata_panic(|| state_suspension_summary(&cross_machine, state))
            .contains("belongs to another machine")
    );
}

#[test]
fn graph_metadata_requires_the_complete_typed_state_flow_set() {
    let machine_symbol = SymbolHandle::from_arena_index(61);
    let first_state = SymbolHandle::from_arena_index(62);
    let missing_state = SymbolHandle::from_arena_index(63);
    let mut program = CheckedTrees::default();
    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Application::run"),
        ..Default::default()
    };
    program.typed.push_machine_state(
        &mut machine,
        psi_checked_trees::state::State {
            symbol: first_state,
            name: Identifier::generated("first"),
            ..Default::default()
        },
    );
    program.typed.push_machine_state(
        &mut machine,
        psi_checked_trees::state::State {
            symbol: missing_state,
            name: Identifier::generated("missing"),
            ..Default::default()
        },
    );
    program.typed.push_machine(machine);
    push_operational_contract(&mut program, machine_symbol, true, false);
    push_operational_flow_state(
        &mut program,
        machine_symbol,
        first_state,
        psi_language_semantics::SuspensionSummary {
            direct_may_suspend: true,
            transitive_may_suspend: true,
        },
        Default::default(),
    );

    assert!(
        metadata_panic(|| machine_suspension_summary(&program, machine_symbol))
            .contains("FlowState fact is missing")
    );
}

#[test]
fn graph_metadata_rejects_missing_duplicate_and_cross_owner_state_reach() {
    let (mut missing, machine, state) = complete_metadata_program();
    missing.facts.service_reaches = Default::default();
    missing.facts.service_reaches.machines.append_to_span(
        &mut missing.facts.service_reaches.root_machines,
        MachineServiceReachRows {
            machine,
            inferred_direct: ServiceReachRowTable::EMPTY_ROW,
            inferred_transitive: ServiceReachRowTable::EMPTY_ROW,
            ..Default::default()
        },
    );
    assert!(
        metadata_panic(|| state_service_reach(&missing, state))
            .contains("state service-reach row is missing")
    );

    let (mut duplicate, machine, state) = complete_metadata_program();
    duplicate.facts.service_reaches = Default::default();
    let state_row = StateServiceReachRows {
        state,
        inferred_direct: ServiceReachRowTable::EMPTY_ROW,
        inferred_transitive: ServiceReachRowTable::EMPTY_ROW,
        ..Default::default()
    };
    let states = duplicate
        .facts
        .service_reaches
        .states
        .insert_many([state_row.clone(), state_row]);
    duplicate.facts.service_reaches.machines.append_to_span(
        &mut duplicate.facts.service_reaches.root_machines,
        MachineServiceReachRows {
            machine,
            inferred_direct: ServiceReachRowTable::EMPTY_ROW,
            inferred_transitive: ServiceReachRowTable::EMPTY_ROW,
            states,
            ..Default::default()
        },
    );
    assert!(
        metadata_panic(|| state_service_reach(&duplicate, state))
            .contains("state service-reach row is duplicated")
    );

    let (mut cross_owner, machine, state) = complete_metadata_program();
    cross_owner.facts.service_reaches = Default::default();
    cross_owner.facts.service_reaches.machines.append_to_span(
        &mut cross_owner.facts.service_reaches.root_machines,
        MachineServiceReachRows {
            machine,
            inferred_direct: ServiceReachRowTable::EMPTY_ROW,
            inferred_transitive: ServiceReachRowTable::EMPTY_ROW,
            ..Default::default()
        },
    );
    push_service_reach(
        &mut cross_owner,
        SymbolHandle::from_arena_index(55),
        state,
        ServiceReachRowTable::EMPTY_ROW,
        ServiceReachRowTable::EMPTY_ROW,
    );
    assert!(
        metadata_panic(|| state_service_reach(&cross_owner, state))
            .contains("belongs to another machine")
    );
}
