//! Task activation plan tests.

use super::{
    Arc, CheckedTrees, Diagnostic, NativeTarget, TaskActivationPlanSet,
    elaborate_task_activation_plans, settle_task_activation_plans,
};
use crate::task_plans::carry_crossings::{
    activation_carry_crossings, exact_activation_carry_subtree, validate_activation_carry_crossing,
};
use crate::task_plans::runtime_requirements::{
    exact_task_machine_blocking, exact_task_machine_suspension, exact_task_runtime_requirement,
    selected_task_runtime_provider,
};
use crate::task_plans::specialization_commitments::{
    exact_task_machine_contract, task_specialization_commitment,
};
use crate::task_plans::stack_layouts::carry_obligations;
use crate::task_plans::start_selections::{
    TaskActivationTargetError, TaskStartSelection, exact_task_activation_target,
    task_start_selections,
};
use language_semantics::{CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension};
use task_plans::{ActivationCarryObligations, TaskStartOperation};

#[test]
fn preservation_mapping_keeps_only_cpu_and_thread_obligations() {
    assert_eq!(
        carry_obligations(CarryPolicy {
            suspension: CarrySuspension::Allowed,
            cpu: CarryCpu::Origin,
            host_thread: CarryHostThread::Origin,
            address: language_semantics::CarryAddress::Stable,
        }),
        ActivationCarryObligations {
            preserve_cpu: true,
            preserve_host_thread: true,
        }
    );
}

fn activation_operational_fixture() -> (CheckedTrees, symbols::SymbolHandle, symbols::SymbolHandle)
{
    let target = symbols::SymbolHandle::from_arena_index(1);
    let unrelated = symbols::SymbolHandle::from_arena_index(2);
    let mut program = CheckedTrees::default();
    program.facts.contract_plans.machines = vec![
        checked_trees::MachineContractPlan {
            machine: target,
            closed_scalar_values: Default::default(),
            crash: Default::default(),
            report_fingerprint: 0x1111,
            commitment: checked_trees::MachineContractCommitment::from_digest([1; 32]),
        },
        checked_trees::MachineContractPlan {
            machine: unrelated,
            closed_scalar_values: Default::default(),
            crash: Default::default(),
            report_fingerprint: 0x2222,
            commitment: checked_trees::MachineContractCommitment::from_digest([2; 32]),
        },
    ];
    program.facts.suspensions.machines = vec![
        checked_trees::MachineSuspensionFact {
            machine: target,
            plan: language_semantics::SuspensionPlan {
                interface: language_semantics::SuspensionInterface::InternalInferred,
                checked_may_suspend: true,
            },
        },
        checked_trees::MachineSuspensionFact {
            machine: unrelated,
            plan: language_semantics::SuspensionPlan {
                interface: language_semantics::SuspensionInterface::PublishedMaySuspend(false),
                checked_may_suspend: false,
            },
        },
    ];
    program.facts.blocking.machines = vec![
        checked_trees::MachineBlockingFact {
            machine: target,
            plan: language_semantics::BlockingPlan {
                interface: language_semantics::BlockingInterface::PublishedMayBlock(false),
                checked_may_block: false,
            },
        },
        checked_trees::MachineBlockingFact {
            machine: unrelated,
            plan: language_semantics::BlockingPlan {
                interface: language_semantics::BlockingInterface::InternalInferred,
                checked_may_block: true,
            },
        },
    ];
    (program, target, unrelated)
}

fn operational_error<T>(result: Result<T, Vec<Diagnostic>>) -> String {
    match result {
        Ok(_) => panic!("invalid exact operational rows must fail closed"),
        Err(diagnostics) => diagnostics
            .first()
            .expect("operational diagnostic")
            .message
            .clone(),
    }
}

#[test]
fn activation_operational_rows_preserve_independent_exact_axes() {
    let (program, target, _) = activation_operational_fixture();

    assert_eq!(
        exact_task_machine_contract(&program, target, "Target")
            .expect("exact contract")
            .report_fingerprint,
        0x1111
    );
    assert!(
        exact_task_machine_suspension(&program, target, "Target")
            .expect("exact suspension")
            .checked_may_suspend
    );
    assert!(
        !exact_task_machine_blocking(&program, target, "Target")
            .expect("exact blocking")
            .checked_may_block
    );
}

#[test]
fn activation_operational_rejects_missing_contract() {
    let (mut program, target, _) = activation_operational_fixture();
    program
        .facts
        .contract_plans
        .machines
        .retain(|plan| plan.machine != target);

    assert!(
        operational_error(exact_task_machine_contract(&program, target, "Target"))
            .contains("no checked machine contract")
    );
}

#[test]
fn activation_operational_rejects_missing_suspension() {
    let (mut program, target, _) = activation_operational_fixture();
    program
        .facts
        .suspensions
        .machines
        .retain(|fact| fact.machine != target);

    assert!(
        operational_error(exact_task_machine_suspension(&program, target, "Target"))
            .contains("no checked suspension plan")
    );
}

#[test]
fn activation_operational_rejects_missing_blocking() {
    let (mut program, target, _) = activation_operational_fixture();
    program
        .facts
        .blocking
        .machines
        .retain(|fact| fact.machine != target);

    assert!(
        operational_error(exact_task_machine_blocking(&program, target, "Target"))
            .contains("no checked blocking plan")
    );
}

#[test]
fn activation_operational_rejects_duplicate_contract() {
    let (mut program, target, _) = activation_operational_fixture();
    let mut duplicate = program.facts.contract_plans.machines[0].clone();
    duplicate.report_fingerprint = 0x3333;
    program.facts.contract_plans.machines.push(duplicate);

    assert!(
        operational_error(exact_task_machine_contract(&program, target, "Target"))
            .contains("duplicate exact checked machine contracts")
    );
}

#[test]
fn activation_operational_rejects_duplicate_suspension() {
    let (mut program, target, _) = activation_operational_fixture();
    let mut duplicate = program.facts.suspensions.machines[0];
    duplicate.plan.checked_may_suspend = false;
    program.facts.suspensions.machines.push(duplicate);

    assert!(
        operational_error(exact_task_machine_suspension(&program, target, "Target"))
            .contains("duplicate exact checked suspension plans")
    );
}

#[test]
fn activation_operational_rejects_duplicate_blocking() {
    let (mut program, target, _) = activation_operational_fixture();
    let mut duplicate = program.facts.blocking.machines[0];
    duplicate.plan.checked_may_block = true;
    program.facts.blocking.machines.push(duplicate);

    assert!(
        operational_error(exact_task_machine_blocking(&program, target, "Target"))
            .contains("duplicate exact checked blocking plans")
    );
}

#[test]
fn activation_operational_ignores_unrelated_duplicate_rows() {
    let (mut program, target, _) = activation_operational_fixture();
    let unrelated_contract = program.facts.contract_plans.machines[1].clone();
    let unrelated_suspension = program.facts.suspensions.machines[1];
    let unrelated_blocking = program.facts.blocking.machines[1];
    program
        .facts
        .contract_plans
        .machines
        .push(unrelated_contract);
    program
        .facts
        .suspensions
        .machines
        .push(unrelated_suspension);
    program.facts.blocking.machines.push(unrelated_blocking);

    assert!(exact_task_machine_contract(&program, target, "Target").is_ok());
    assert!(exact_task_machine_suspension(&program, target, "Target").is_ok());
    assert!(exact_task_machine_blocking(&program, target, "Target").is_ok());
}

struct ActivationRequirementFixture {
    program: CheckedTrees,
    selection: TaskStartSelection,
    other_owner: symbols::SymbolHandle,
    other_requirement: symbols::SymbolHandle,
    private_owner: symbols::SymbolHandle,
    private_requirement: symbols::SymbolHandle,
}

fn activation_requirement_fixture(duplicate_requirement: bool) -> ActivationRequirementFixture {
    let owner = symbols::SymbolHandle::from_arena_index(1);
    let requirement = symbols::SymbolHandle::from_arena_index(2);
    let other_owner = symbols::SymbolHandle::from_arena_index(3);
    let other_requirement = symbols::SymbolHandle::from_arena_index(4);
    let private_owner = symbols::SymbolHandle::from_arena_index(5);
    let private_requirement = symbols::SymbolHandle::from_arena_index(6);
    let mut program = CheckedTrees::default();

    let mut task_runtime = checked_trees::trait_definition::TraitDefinition {
        symbol: owner,
        is_boundary: true,
        name: checked_trees::name::Identifier::generated("core::TaskRuntime"),
        ..Default::default()
    };
    program.typed.push_trait_machine_signature(
        &mut task_runtime,
        checked_trees::signature::StateSignature {
            symbol: requirement,
            name: checked_trees::name::Identifier::generated("start"),
            ..Default::default()
        },
    );
    if duplicate_requirement {
        program.typed.push_trait_machine_signature(
            &mut task_runtime,
            checked_trees::signature::StateSignature {
                symbol: requirement,
                name: checked_trees::name::Identifier::generated("duplicate"),
                ..Default::default()
            },
        );
    }
    program.typed.push_trait_definition(task_runtime);

    let mut other = checked_trees::trait_definition::TraitDefinition {
        symbol: other_owner,
        is_boundary: true,
        name: checked_trees::name::Identifier::generated("other::TaskRuntime"),
        ..Default::default()
    };
    program.typed.push_trait_machine_signature(
        &mut other,
        checked_trees::signature::StateSignature {
            symbol: other_requirement,
            name: checked_trees::name::Identifier::generated("start"),
            ..Default::default()
        },
    );
    program.typed.push_trait_definition(other);

    let mut private = checked_trees::trait_definition::TraitDefinition {
        symbol: private_owner,
        is_boundary: false,
        name: checked_trees::name::Identifier::generated("PrivateRuntime"),
        ..Default::default()
    };
    program.typed.push_trait_machine_signature(
        &mut private,
        checked_trees::signature::StateSignature {
            symbol: private_requirement,
            name: checked_trees::name::Identifier::generated("start"),
            ..Default::default()
        },
    );
    program.typed.push_trait_definition(private);

    ActivationRequirementFixture {
        program,
        selection: TaskStartSelection {
            requirement_owner: owner,
            requirement,
            target_machine: symbols::SymbolHandle::from_arena_index(7),
            target_entry: symbols::SymbolHandle::from_arena_index(8),
            report_fingerprint: 1,
            operation: TaskStartOperation::Start,
        },
        other_owner,
        other_requirement,
        private_owner,
        private_requirement,
    }
}

fn requirement_error(program: &CheckedTrees, selection: &TaskStartSelection) -> String {
    match exact_task_runtime_requirement(program, selection) {
        Ok(_) => panic!("invalid exact requirement custody must fail closed"),
        Err(diagnostics) => diagnostics
            .first()
            .expect("requirement diagnostic")
            .message
            .clone(),
    }
}

#[test]
fn activation_requirement_retains_exact_boundary_owner_and_signature() {
    let fixture = activation_requirement_fixture(false);
    let (owner, signature, identity) =
        exact_task_runtime_requirement(&fixture.program, &fixture.selection)
            .expect("exact requirement");

    assert_eq!(owner.symbol, fixture.selection.requirement_owner);
    assert_eq!(signature.symbol, fixture.selection.requirement);
    assert!(identity.contains("core::TaskRuntime::start"));
}

#[test]
fn activation_requirement_rejects_missing_owner() {
    let mut fixture = activation_requirement_fixture(false);
    fixture.selection.requirement_owner = symbols::SymbolHandle::invalid();

    assert!(requirement_error(&fixture.program, &fixture.selection).contains("one exact"));
}

#[test]
fn activation_requirement_rejects_duplicate_owner() {
    let mut fixture = activation_requirement_fixture(false);
    fixture
        .program
        .typed
        .push_trait_definition(checked_trees::trait_definition::TraitDefinition {
            symbol: fixture.selection.requirement_owner,
            is_boundary: true,
            name: checked_trees::name::Identifier::generated("duplicate::TaskRuntime"),
            ..Default::default()
        });

    assert!(requirement_error(&fixture.program, &fixture.selection).contains("uniquely"));
}

#[test]
fn activation_requirement_rejects_non_boundary_owner() {
    let mut fixture = activation_requirement_fixture(false);
    fixture.selection.requirement_owner = fixture.private_owner;
    fixture.selection.requirement = fixture.private_requirement;

    assert!(requirement_error(&fixture.program, &fixture.selection).contains("boundary"));
}

#[test]
fn activation_requirement_rejects_missing_owned_signature() {
    let mut fixture = activation_requirement_fixture(false);
    fixture.selection.requirement = symbols::SymbolHandle::invalid();

    assert!(
        requirement_error(&fixture.program, &fixture.selection)
            .contains("belong to its exact retained owner")
    );
}

#[test]
fn activation_requirement_rejects_duplicate_owned_signature() {
    let fixture = activation_requirement_fixture(true);

    assert!(
        requirement_error(&fixture.program, &fixture.selection)
            .contains("resolve uniquely within its exact owner")
    );
}

#[test]
fn activation_requirement_rejects_cross_owner_signature_drift() {
    let mut fixture = activation_requirement_fixture(false);
    fixture.selection.requirement_owner = fixture.other_owner;

    assert!(
        requirement_error(&fixture.program, &fixture.selection)
            .contains("belong to its exact retained owner")
    );
}

#[test]
fn activation_requirement_ignores_unrelated_trait_and_signature() {
    let fixture = activation_requirement_fixture(false);
    assert_ne!(fixture.other_owner, fixture.selection.requirement_owner);
    assert_ne!(fixture.other_requirement, fixture.selection.requirement);

    let (owner, signature, _) =
        exact_task_runtime_requirement(&fixture.program, &fixture.selection)
            .expect("unrelated retained trait does not perturb exact owner");
    assert_eq!(owner.symbol, fixture.selection.requirement_owner);
    assert_eq!(signature.symbol, fixture.selection.requirement);
}

fn activation_target_fixture() -> (
    CheckedTrees,
    symbols::SymbolHandle,
    symbols::SymbolHandle,
    symbols::SymbolHandle,
    symbols::SymbolHandle,
) {
    let first_machine = symbols::SymbolHandle::from_arena_index(1);
    let first_entry = symbols::SymbolHandle::from_arena_index(2);
    let second_machine = symbols::SymbolHandle::from_arena_index(3);
    let second_entry = symbols::SymbolHandle::from_arena_index(4);
    let mut program = CheckedTrees::default();

    let mut first = checked_trees::machine::Machine {
        symbol: first_machine,
        name: checked_trees::name::Identifier::generated("First::run"),
        ..Default::default()
    };
    program.typed.push_machine_state(
        &mut first,
        checked_trees::state::State {
            symbol: first_entry,
            name: checked_trees::name::Identifier::generated("run"),
            ..Default::default()
        },
    );
    program.typed.push_machine(first);

    let mut second = checked_trees::machine::Machine {
        symbol: second_machine,
        name: checked_trees::name::Identifier::generated("Second::run"),
        ..Default::default()
    };
    program.typed.push_machine_state(
        &mut second,
        checked_trees::state::State {
            symbol: second_entry,
            name: checked_trees::name::Identifier::generated("run"),
            ..Default::default()
        },
    );
    program.typed.push_machine(second);

    (
        program,
        first_machine,
        first_entry,
        second_machine,
        second_entry,
    )
}

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

fn activation_crossing_validation_fixture() -> (
    CheckedTrees,
    symbols::SymbolHandle,
    symbols::SymbolHandle,
    symbols::SymbolHandle,
) {
    let root = symbols::SymbolHandle::from_arena_index(1);
    let root_state = symbols::SymbolHandle::from_arena_index(2);
    let child = symbols::SymbolHandle::from_arena_index(3);
    let child_state = symbols::SymbolHandle::from_arena_index(4);
    let field = symbols::SymbolHandle::from_arena_index(5);
    let data = symbols::SymbolHandle::from_arena_index(6);
    let mut program = CheckedTrees::default();
    let contained_type =
        program
            .typed
            .type_reference_table
            .insert(checked_trees::types::TypeReferenceNode::Named {
                symbol: data,
                name: checked_trees::name::Identifier::generated("ChildData"),
            });

    let mut root_machine = checked_trees::machine::Machine {
        symbol: root,
        name: checked_trees::name::Identifier::generated("Root::run"),
        ..Default::default()
    };
    let mut root_state_definition = checked_trees::state::State {
        symbol: root_state,
        name: checked_trees::name::Identifier::generated("run"),
        ..Default::default()
    };
    program.typed.statement_table.push_statement(
        &mut root_state_definition.statement_nodes,
        Default::default(),
    );
    program
        .typed
        .push_machine_state(&mut root_machine, root_state_definition);
    program.typed.push_machine(root_machine);

    let mut child_machine = checked_trees::machine::Machine {
        symbol: child,
        name: checked_trees::name::Identifier::generated("Child::run"),
        ..Default::default()
    };
    let mut child_state_definition = checked_trees::state::State {
        symbol: child_state,
        name: checked_trees::name::Identifier::generated("run"),
        ..Default::default()
    };
    program.typed.statement_table.push_statement(
        &mut child_state_definition.statement_nodes,
        Default::default(),
    );
    program
        .typed
        .push_machine_state(&mut child_machine, child_state_definition);
    program.typed.push_machine(child_machine);

    let mut calls = arena::HandleSpan::empty();
    program.facts.flow.control.calls.append_to_span(
        &mut calls,
        checked_trees::FlowCallFact {
            statement_index: 0,
            call_ordinal: 0,
            target_symbol: root_state,
            suspension: language_semantics::SuspensionSummary {
                direct_may_suspend: true,
                transitive_may_suspend: false,
            },
            ..Default::default()
        },
    );
    program.facts.flow.control.calls.append_to_span(
        &mut calls,
        checked_trees::FlowCallFact {
            statement_index: 0,
            call_ordinal: 1,
            target_symbol: child_state,
            suspension: language_semantics::SuspensionSummary {
                direct_may_suspend: false,
                transitive_may_suspend: true,
            },
            ..Default::default()
        },
    );
    program
        .facts
        .flow
        .control
        .states
        .append(checked_trees::FlowStateFact {
            machine_symbol: child,
            state_symbol: child_state,
            calls,
            ..Default::default()
        });

    let targets = program
        .facts
        .carry
        .contained_targets
        .insert_many([checked_trees::ContainedMachineTargetFact { machine: child }]);
    let fields = program.facts.carry.contained_fields.insert_many([
        checked_trees::ContainedMachineFieldFact {
            field,
            data,
            type_reference: contained_type,
            targets,
        },
    ]);
    program
        .facts
        .carry
        .machine_topologies
        .insert(checked_trees::MachineCarryTopologyFact {
            machine: root,
            fields,
        });
    program
        .facts
        .carry
        .machine_topologies
        .insert(checked_trees::MachineCarryTopologyFact {
            machine: child,
            fields: arena::HandleSpan::empty(),
        });
    program
        .facts
        .carry
        .suspension_crossings
        .push(checked_trees::SuspensionCrossingCarryFact {
            machine: child,
            state: child_state,
            statement_index: 0,
            call_ordinal: 0,
            target: root_state,
            receiver: None,
            effective: CarryPolicy {
                suspension: CarrySuspension::Allowed,
                cpu: CarryCpu::Origin,
                host_thread: CarryHostThread::Any,
                address: language_semantics::CarryAddress::Movable,
            },
            live_values: Vec::new(),
        });
    program
        .facts
        .carry
        .suspension_crossings
        .push(checked_trees::SuspensionCrossingCarryFact {
            machine: child,
            state: child_state,
            statement_index: 0,
            call_ordinal: 1,
            target: child_state,
            receiver: None,
            effective: CarryPolicy::PERMISSIVE,
            live_values: Vec::new(),
        });
    (program, root, root_state, child_state)
}

fn crossing_error(program: &CheckedTrees, root: symbols::SymbolHandle) -> String {
    match activation_carry_crossings(program, root) {
        Ok(_) => panic!("invalid crossing must fail closed"),
        Err(diagnostics) => diagnostics
            .first()
            .expect("crossing diagnostic")
            .message
            .clone(),
    }
}

#[test]
fn activation_crossings_include_contained_machine_crossings() {
    let (program, root, _, _) = activation_crossing_validation_fixture();
    let child_policy = program.facts.carry.suspension_crossings[0].effective;
    let crossings = activation_carry_crossings(&program, root).expect("exact crossing custody");

    assert!(crossings.root.is_empty());
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

fn topology_error(program: &CheckedTrees, root: symbols::SymbolHandle) -> String {
    match exact_activation_carry_subtree(program, root) {
        Ok(_) => panic!("invalid topology must fail closed"),
        Err(diagnostics) => diagnostics
            .first()
            .expect("topology diagnostic")
            .message
            .clone(),
    }
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

#[test]
fn activation_crossings_reject_missing_machine() {
    let (program, _, _, _) = activation_crossing_validation_fixture();
    let mut crossing = program.facts.carry.suspension_crossings[0].clone();
    crossing.machine = symbols::SymbolHandle::invalid();
    let diagnostics = validate_activation_carry_crossing(&program, &crossing)
        .expect_err("missing crossing machine must fail closed");
    assert!(diagnostics[0].message.contains("exact typed machine"));
}

#[test]
fn activation_crossings_reject_cross_machine_state() {
    let (mut program, root, root_state, _) = activation_crossing_validation_fixture();
    program.facts.carry.suspension_crossings[0].state = root_state;
    assert!(crossing_error(&program, root).contains("belong to its exact typed machine"));
}

#[test]
fn activation_crossings_reject_out_of_range_statement() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    program.facts.carry.suspension_crossings[0].statement_index = 1;
    assert!(crossing_error(&program, root).contains("statement must belong"));
}

#[test]
fn activation_crossings_reject_missing_flow_state() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    program.facts.flow.control.states = Default::default();
    assert!(crossing_error(&program, root).contains("one exact checked flow state"));
}

#[test]
fn activation_crossings_reject_ambiguous_flow_state() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    let duplicate = program
        .facts
        .flow
        .control
        .states
        .iter()
        .next()
        .expect("flow state")
        .1
        .clone();
    program.facts.flow.control.states.append(duplicate);
    assert!(crossing_error(&program, root).contains("exactly one checked flow state"));
}

#[test]
fn activation_crossings_reject_invalid_call_span() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    program.facts.flow.control.states.for_each_mut(|_, state| {
        state.calls = arena::HandleSpan::from_parts(arena::Handle::invalid(), 1);
    });
    assert!(crossing_error(&program, root).contains("exact valid call span"));
}

#[test]
fn activation_crossings_reject_missing_call_coordinate() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    program.facts.carry.suspension_crossings[0].call_ordinal = 2;
    assert!(crossing_error(&program, root).contains("one exact checked flow call"));
}

#[test]
fn activation_crossings_reject_ambiguous_call_coordinate() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    let call = program
        .facts
        .flow
        .control
        .calls
        .iter()
        .next()
        .expect("flow call")
        .1
        .clone();
    let calls = program
        .facts
        .flow
        .control
        .calls
        .insert_many([call.clone(), call]);
    program
        .facts
        .flow
        .control
        .states
        .for_each_mut(|_, state| state.calls = calls);
    assert!(crossing_error(&program, root).contains("exactly one checked flow call"));
}

#[test]
fn activation_crossings_reject_target_drift() {
    let (mut program, root, _, child_state) = activation_crossing_validation_fixture();
    program.facts.carry.suspension_crossings[0].target = child_state;
    assert!(crossing_error(&program, root).contains("exact checked call target"));
}

#[test]
fn activation_crossings_reject_missing_typed_target() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    program.facts.carry.suspension_crossings[0].target = symbols::SymbolHandle::invalid();
    program
        .facts
        .flow
        .control
        .calls
        .for_each_mut(|_, call| call.target_symbol = symbols::SymbolHandle::invalid());
    assert!(crossing_error(&program, root).contains("target must name an exact typed state"));
}

#[test]
fn activation_crossings_reject_non_suspending_call() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    program
        .facts
        .flow
        .control
        .calls
        .for_each_mut(|_, call| call.suspension = Default::default());
    assert!(crossing_error(&program, root).contains("may-suspend checked call"));
}

#[test]
fn activation_crossings_reject_duplicate_coordinate() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    let duplicate = program.facts.carry.suspension_crossings[0].clone();
    program.facts.carry.suspension_crossings.push(duplicate);
    assert!(crossing_error(&program, root).contains("one row per exact call coordinate"));
}

fn concrete_task_start_fixture() -> (
    CheckedTrees,
    effects::SelectedProviderPlanFacts,
    Vec<effects::provider_plan::ProviderPlan>,
) {
    let source = r#"
        data Task<T> [linear] { provider: u64; activation: u64; }
        machine Task::settle<T>(self) {}
        data StartRejection { code: i32; }
        data StartOutcome<T, Arguments> {
            case Started(task: Task<T>);
            case Rejected(arguments: Arguments, reason: StartRejection);
        }
        boundary trait TaskRuntime {
            machine start<T, Arguments, machine Target>(
                &self,
                arguments: Arguments
            ) -> Task<T>
            where machine Target(arguments: Arguments) -> T suspends; blocks;
            ensures true;
            machine try_start<T, Arguments, machine Target>(
                &self,
                arguments: Arguments
            ) -> StartOutcome<T, Arguments>
            where machine Target(arguments: Arguments) -> T suspends; blocks;
            ensures true;
        }

        data LocalTaskRuntime { }
        LocalTaskRuntimeTaskRuntime: LocalTaskRuntime satisfies TaskRuntime;
        machine LocalTaskRuntime::start<T, Arguments, machine Target>(
            &self,
            arguments: Arguments
        ) -> Task<T>
        where machine Target(arguments: Arguments) -> T suspends; blocks;
        satisfies TaskRuntime::start
        via Binding::CompilerIntrinsic;
        machine LocalTaskRuntime::try_start<T, Arguments, machine Target>(
            &self,
            arguments: Arguments
        ) -> StartOutcome<T, Arguments>
        where machine Target(arguments: Arguments) -> T suspends; blocks;
        satisfies TaskRuntime::try_start
        via Binding::CompilerIntrinsic;

        pub boundary data Sleeper;
        boundary machine Sleeper::park(token: i32) suspends;
        data Job { value: i32; }
        data Worker {}
        machine Worker::run(job: Job) -> i32 suspends; {
            let value: i32 = job.value;
            suspend Sleeper::park(value);
            value
        }
        data Main { runtime: &TaskRuntime; }
        machine Main::run(&mut self) reaches TaskRuntime {
            let job: Job = Job { value: 7 };
            let task: Task<i32> = self.runtime.start<Worker::run>(job);
            Task::settle(task);
            let retry: Job = Job { value: 9 };
            let outcome: StartOutcome<i32, Job> =
                self.runtime.try_start<Worker::run>(retry);
            let retry_task: Task<i32> = outcome.task;
            Task::settle(retry_task);
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let provider_plans = crate::provider_planning::derive_satisfies_plans(&typed, None);
    assert_eq!(provider_plans.len(), 1);
    assert!(
        crate::provider_planning::validate_provider_plan_candidates(&typed, &provider_plans,)
            .is_empty()
    );
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        &provider_plans,
        &[provider_plans[0].name.clone()],
    )
    .expect("select complete TaskRuntime provider");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("check and specialize task start");

    (checked, selected, provider_plans)
}

#[test]
fn compiler_task_activation_settlement_borrows_shared_program_and_commits_two_rows() {
    let (checked, selected, _) = concrete_task_start_fixture();
    let original_sidecar = Arc::new(TaskActivationPlanSet::default());
    let mut retained = Arc::clone(&original_sidecar);

    settle_task_activation_plans(
        &mut retained,
        &checked,
        &selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect("shared checked custody should produce the complete activation sidecar");

    assert!(!Arc::ptr_eq(&original_sidecar, &retained));
    assert_eq!(retained.as_slice().len(), 2);
    assert!(
        retained
            .as_slice()
            .iter()
            .any(|activation| { activation.operation == TaskStartOperation::Start })
    );
    assert!(
        retained
            .as_slice()
            .iter()
            .any(|activation| { activation.operation == TaskStartOperation::TryStart })
    );
}

#[test]
fn compact_equal_specialization_substitution_changes_authoritative_commitment() {
    let (mut checked, selected, _) = concrete_task_start_fixture();
    let baseline_start = task_start_selections(&checked)
        .expect("baseline task selections")
        .into_iter()
        .find(|selection| selection.operation == TaskStartOperation::Start)
        .expect("baseline start selection");
    let target_specialization_index = checked.typed.machine_specializations.len();
    checked
        .typed
        .machine_specializations
        .push(typed_trees::typed_trees::MachineSpecialization {
            template: baseline_start.target_machine,
            instance: baseline_start.target_machine,
            type_argument_identities: vec!["exact::Baseline".to_owned()],
            report_fingerprint: 0x4455,
            ..Default::default()
        });
    let baseline_runtime = selected_task_runtime_provider(&checked, &selected, &baseline_start)
        .expect("baseline selected task runtime");
    let (baseline_machine, baseline_entry) = exact_task_activation_target(
        &checked,
        baseline_start.target_machine,
        baseline_start.target_entry,
    )
    .expect("baseline exact task target");
    let baseline_contract = exact_task_machine_contract(
        &checked,
        baseline_machine.symbol,
        baseline_machine.name.as_str(),
    )
    .expect("baseline exact task contract");
    let baseline_commitment = task_specialization_commitment(
        &checked,
        &baseline_start,
        baseline_machine,
        baseline_entry,
        baseline_contract,
        baseline_runtime.requirement_identity.as_str(),
    )
    .expect("baseline task specialization commitment");

    let mut substituted = checked.clone();
    substituted.typed.machine_specializations[target_specialization_index]
        .type_argument_identities[0] = "exact::Substituted".to_owned();
    assert_eq!(
        substituted.machine_specializations[target_specialization_index].report_fingerprint,
        checked.machine_specializations[target_specialization_index].report_fingerprint,
    );
    let (changed_machine, changed_entry) = exact_task_activation_target(
        &substituted,
        baseline_start.target_machine,
        baseline_start.target_entry,
    )
    .expect("changed exact task target");
    let changed_contract = exact_task_machine_contract(
        &substituted,
        changed_machine.symbol,
        changed_machine.name.as_str(),
    )
    .expect("changed exact task contract");
    let changed_commitment = task_specialization_commitment(
        &substituted,
        &baseline_start,
        changed_machine,
        changed_entry,
        changed_contract,
        baseline_runtime.requirement_identity.as_str(),
    )
    .expect("changed task specialization commitment");

    assert_ne!(baseline_commitment, changed_commitment);
}

#[test]
fn compiler_task_activation_rejection_preserves_prior_sidecar_identity() {
    let (mut checked, selected, _) = concrete_task_start_fixture();
    let retained =
        elaborate_task_activation_plans(&checked, &selected, NativeTarget::macos_arm64(), &[])
            .expect("fixture should produce a retained activation sidecar");
    let target = retained
        .as_slice()
        .iter()
        .find(|activation| activation.operation == TaskStartOperation::Start)
        .expect("fixture start activation")
        .target_machine;
    checked
        .facts
        .suspensions
        .machines
        .retain(|fact| fact.machine != target);
    let original_sidecar = Arc::new(retained);
    let mut retained = Arc::clone(&original_sidecar);

    let diagnostics = settle_task_activation_plans(
        &mut retained,
        &checked,
        &selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect_err("missing exact suspension evidence must reject settlement");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "task activation target `Worker::run` has no checked suspension plan"
    );
    assert!(Arc::ptr_eq(&original_sidecar, &retained));
}

#[test]
fn compiler_task_activation_settlement_commits_canonical_empty_sidecar() {
    let original_sidecar = Arc::new(TaskActivationPlanSet::default());
    let checked = CheckedTrees::default();
    let selected = effects::SelectedProviderPlanFacts::default();
    let mut retained = Arc::clone(&original_sidecar);

    settle_task_activation_plans(
        &mut retained,
        &checked,
        &selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect("an empty checked program should produce an empty activation sidecar");

    assert!(!Arc::ptr_eq(&original_sidecar, &retained));
    assert_eq!(retained.as_ref(), &TaskActivationPlanSet::default());
}

#[test]
fn concrete_task_start_specialization_elaborates_a_validated_plan() {
    let (checked, selected, provider_plans) = concrete_task_start_fixture();

    let mut foreign_leaf_plan = provider_plans[0].clone();
    foreign_leaf_plan.schema.trait_name = "other::TaskRuntime".to_owned();
    let foreign_leaf_selected = effects::SelectedProviderPlanFacts::from_selection(
        &[foreign_leaf_plan.clone()],
        &[foreign_leaf_plan.name.clone()],
    )
    .expect("same-leaf foreign schema remains a structurally complete plan");
    let diagnostics = elaborate_task_activation_plans(
        &checked,
        &foreign_leaf_selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect_err("same-leaf foreign schema must not satisfy exact TaskRuntime owner");
    assert!(
        diagnostics[0]
            .message
            .contains("no retained selected provider plan")
    );

    let mut wrong_method_owner_plan = provider_plans[0].clone();
    for method in &mut wrong_method_owner_plan.schema.methods {
        method.requirement_owner = "other::TaskRuntime".to_owned();
    }
    let wrong_method_owner_selected = effects::SelectedProviderPlanFacts::from_selection(
        &[wrong_method_owner_plan.clone()],
        &[wrong_method_owner_plan.name.clone()],
    )
    .expect("method-owner drift remains structurally complete");
    let diagnostics = elaborate_task_activation_plans(
        &checked,
        &wrong_method_owner_selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect_err("method owner drift must not satisfy exact TaskRuntime requirement");
    assert!(
        diagnostics[0]
            .message
            .contains("no retained selected provider plan")
    );

    let task_activations =
        elaborate_task_activation_plans(&checked, &selected, NativeTarget::macos_arm64(), &[])
            .expect("elaborate activation plan");
    let activations = task_activations.as_slice();
    assert_eq!(activations.len(), 2);
    let activation = activations
        .iter()
        .find(|activation| activation.operation == TaskStartOperation::Start)
        .expect("start activation plan");
    let target_machine_symbol = activation.target_machine;
    assert!(
        activations
            .iter()
            .any(|activation| activation.operation == TaskStartOperation::TryStart)
    );
    let target = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == activation.target_machine)
        .expect("target machine");
    assert_eq!(target.name.as_str(), "Worker::run");
    let plan = activation.plan.candidate();
    assert_eq!(plan.stack_plan.bytes, 16);
    assert_eq!(plan.stack_plan.alignment, 8);
    assert!(plan.may_suspend);
    assert!(!plan.may_block);
    assert_eq!(plan.canonical_suspension_crossings.len(), 1);
    assert_eq!(plan.carry_obligations, ActivationCarryObligations::none());
    assert!(plan.cancellation_required);
    assert_ne!(plan.machine_contract.normalized_identity(), 0);
    assert_ne!(plan.argument_layout.normalized_identity(), 0);
    assert_ne!(plan.terminal_outcome_layout.normalized_identity(), 0);
    assert_ne!(
        activation.plan.normalized_identity().normalized_identity(),
        0
    );
    assert_eq!(
        activation.selected_runtime.provider_plan_name,
        "LocalTaskRuntime::satisfies::TaskRuntime"
    );
    assert_eq!(
        activation.selected_runtime.runtime.normalized_identity(),
        selected
            .plans()
            .first()
            .expect("selected runtime plan")
            .report_fingerprint()
    );
    assert!(
        activation
            .selected_runtime
            .requirement_identity
            .contains("TaskRuntime")
    );

    let manifest = visualizations::task_activation_manifest_json(&checked, &task_activations);
    assert!(manifest.contains("\"operation\": \"start\""));
    assert!(manifest.contains("\"operation\": \"try_start\""));
    assert!(manifest.contains("\"target_machine\": \"Worker::run\""));
    assert!(manifest.contains("\"selected_runtime\": {"));
    assert!(manifest.contains("\"provider_plan\": \"LocalTaskRuntime::satisfies::TaskRuntime\""));
    assert!(manifest.contains("\"stack_plan\": {\"bytes\": 16, \"alignment\": 8"));
    assert!(manifest.contains("\"canonical_suspension_crossings\": ["));
    assert!(manifest.contains(
        "\"cpu_thread_preservation\": {\"preserve_cpu\": false, \"preserve_host_thread\": false}"
    ));
    assert!(manifest.contains("\"cancellation_required\": true"));
    assert!(manifest.contains("\"activation_plan_id\": \"0x"));
    assert!(!manifest.contains("\"runtime_admission\""));

    let mut missing_suspension = checked.clone();
    missing_suspension
        .facts
        .suspensions
        .machines
        .retain(|fact| fact.machine != activation.target_machine);
    let diagnostics = elaborate_task_activation_plans(
        &missing_suspension,
        &selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect_err("missing exact target suspension facts must fail closed");
    assert_eq!(
        diagnostics[0].message,
        "task activation target `Worker::run` has no checked suspension plan"
    );

    let mut missing_blocking = checked.clone();
    missing_blocking
        .facts
        .blocking
        .machines
        .retain(|fact| fact.machine != activation.target_machine);
    let diagnostics = elaborate_task_activation_plans(
        &missing_blocking,
        &selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect_err("missing exact target blocking facts must fail closed");
    assert_eq!(
        diagnostics[0].message,
        "task activation target `Worker::run` has no checked blocking plan"
    );

    let unrelated_machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol != target_machine_symbol)
        .expect("unrelated checked machine")
        .symbol;

    let mut missing_carry = checked.clone();
    missing_carry
        .facts
        .carry
        .activation_wide_carry
        .retain(|fact| fact.machine != target_machine_symbol);
    let diagnostics = elaborate_task_activation_plans(
        &missing_carry,
        &selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect_err("missing exact target carry envelope must fail closed");
    assert_eq!(
        diagnostics[0].message,
        "task activation target `Worker::run` has no exact activation-wide CPU/thread carry envelope"
    );

    let mut duplicate_carry = checked.clone();
    let duplicate = duplicate_carry
        .facts
        .carry
        .activation_wide_carry
        .iter()
        .find(|fact| fact.machine == target_machine_symbol)
        .expect("target carry envelope")
        .clone();
    duplicate_carry
        .facts
        .carry
        .activation_wide_carry
        .push(duplicate);
    let diagnostics = elaborate_task_activation_plans(
        &duplicate_carry,
        &selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect_err("duplicate exact target carry envelope must fail closed");
    assert_eq!(
        diagnostics[0].message,
        "task activation target `Worker::run` has duplicate exact activation-wide CPU/thread carry envelopes"
    );

    let mut incomplete_carry = checked.clone();
    incomplete_carry
        .facts
        .carry
        .activation_wide_carry
        .iter_mut()
        .find(|fact| fact.machine == target_machine_symbol)
        .expect("target carry envelope")
        .analysis_complete = false;
    let diagnostics = elaborate_task_activation_plans(
        &incomplete_carry,
        &selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect_err("incomplete exact target carry envelope must fail closed");
    assert_eq!(
        diagnostics[0].message,
        "task activation target `Worker::run` has incomplete activation-wide CPU/thread carry analysis"
    );

    let mut authoritative_carry = checked.clone();
    let target_carry = authoritative_carry
        .facts
        .carry
        .activation_wide_carry
        .iter_mut()
        .find(|fact| fact.machine == target_machine_symbol)
        .expect("target carry envelope");
    target_carry.effective.cpu = CarryCpu::Origin;
    target_carry.effective.host_thread = CarryHostThread::Origin;
    let authoritative = elaborate_task_activation_plans(
        &authoritative_carry,
        &selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect("exact checked target envelope remains the sole carry authority");
    let authoritative_plan = authoritative
        .as_slice()
        .iter()
        .find(|activation| activation.operation == TaskStartOperation::Start)
        .expect("authoritative start activation")
        .plan
        .candidate();
    assert_eq!(
        authoritative_plan.carry_obligations,
        ActivationCarryObligations {
            preserve_cpu: true,
            preserve_host_thread: true,
        }
    );
    assert_eq!(authoritative_plan.stack_plan, plan.stack_plan);
    assert_eq!(
        authoritative_plan.canonical_suspension_crossings,
        plan.canonical_suspension_crossings,
    );

    let mut unrelated_carry = checked.clone();
    let mut unrelated = unrelated_carry
        .facts
        .carry
        .activation_wide_carry
        .iter()
        .find(|fact| fact.machine == target_machine_symbol)
        .expect("target carry envelope")
        .clone();
    unrelated.machine = unrelated_machine;
    unrelated_carry
        .facts
        .carry
        .activation_wide_carry
        .push(unrelated);
    assert_eq!(
        elaborate_task_activation_plans(
            &unrelated_carry,
            &selected,
            NativeTarget::macos_arm64(),
            &[],
        )
        .expect("unrelated carry envelope must not perturb exact target selection"),
        task_activations,
    );

    let mut unrelated_only = checked.clone();
    unrelated_only
        .facts
        .carry
        .activation_wide_carry
        .iter_mut()
        .find(|fact| fact.machine == target_machine_symbol)
        .expect("target carry envelope")
        .machine = unrelated_machine;
    let diagnostics = elaborate_task_activation_plans(
        &unrelated_only,
        &selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect_err("unrelated carry envelope must not satisfy exact target selection");
    assert_eq!(
        diagnostics[0].message,
        "task activation target `Worker::run` has no exact activation-wide CPU/thread carry envelope"
    );
}
