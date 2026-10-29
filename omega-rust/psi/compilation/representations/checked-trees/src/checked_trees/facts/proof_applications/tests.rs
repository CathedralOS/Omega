//! Graph controls moved with the former driver-local application traversal.
//! These construct graph records, not source programs or checked judgments.

use super::*;
use flow_effects::{CallOperational, CleanupDependency, MachineOperational, StateOperational};

fn symbol(ordinal: u32) -> SymbolHandle {
    SymbolHandle::from_parts(ordinal, 1)
}

fn machine(plan: &mut ProofApplicationPlan, ordinal: u32, scope: Option<&str>) {
    plan.machines.insert(ProofApplicationMachine {
        machine: symbol(ordinal),
        authored_target: scope.map(str::to_owned),
        source_target: scope.map(str::to_owned),
        product_selected: true,
        ..Default::default()
    });
}

#[test]
fn family_lookup_keeps_exact_members_and_missing_targets() {
    let mut plan = ProofApplicationPlan::default();
    let members = plan.target_members.insert_many([
        ProofApplicationTargetMember {
            target: "macos_arm64".into(),
            machine: symbol(2),
        },
        ProofApplicationTargetMember {
            target: "windows_x86_64".into(),
            machine: symbol(3),
        },
    ]);
    plan.machines.insert(ProofApplicationMachine {
        machine: symbol(1),
        family_interface: true,
        family_members: members,
        product_selected: true,
        ..Default::default()
    });
    machine(&mut plan, 2, Some("macos_arm64"));
    machine(&mut plan, 3, Some("windows_x86_64"));
    let head = plan.called_machine(symbol(1)).expect("family head");
    let member = plan.called_machine(symbol(2)).expect("exact member");
    assert_eq!(
        plan.target_machine(head, "windows_x86_64")
            .map(|body| body.machine),
        Some(symbol(3))
    );
    assert!(plan.target_machine(head, "linux_arm64").is_none());
    assert!(plan.target_machine(member, "windows_x86_64").is_none());
    assert_eq!(
        plan.target_machine(member, "macos_arm64")
            .map(|body| body.machine),
        Some(symbol(2))
    );
}

fn recorded_calls() -> (ProofApplicationPlan, OperationalPlan) {
    let mut plan = ProofApplicationPlan::default();
    machine(&mut plan, 1, Some("windows_x86_64"));
    for ordinal in 2..=8 {
        machine(&mut plan, ordinal, None);
    }
    machine(&mut plan, 9, Some("macos_arm64"));
    let mut operational = OperationalPlan::default();
    let bindings = operational
        .static_machine_bindings
        .insert_many([StaticMachineCallBinding {
            parameter: symbol(10),
            selected: symbol(3),
            ..Default::default()
        }]);
    let root_calls = operational.calls.insert_many([
        CallOperational {
            target_machine_symbol: symbol(2),
            static_machine_bindings: bindings,
            ..Default::default()
        },
        CallOperational {
            target_machine_symbol: symbol(6),
            ..Default::default()
        },
    ]);
    let callback = operational.calls.insert_many([CallOperational {
        static_machine_parameter: symbol(10),
        ..Default::default()
    }]);
    let root_states = operational.states.insert_many([StateOperational {
        calls: root_calls,
        ..Default::default()
    }]);
    let helper_states = operational.states.insert_many([StateOperational {
        calls: callback,
        ..Default::default()
    }]);
    operational.root_machines = operational.machines.insert_many([
        MachineOperational {
            symbol: symbol(1),
            states: root_states,
            ..Default::default()
        },
        MachineOperational {
            symbol: symbol(2),
            states: helper_states,
            ..Default::default()
        },
    ]);
    operational.cleanup_dependencies.insert(CleanupDependency {
        caller_machine: symbol(1),
        target_machine: symbol(5),
        ..Default::default()
    });
    (plan, operational)
}

#[test]
fn supplemental_membership_follows_bindings_cleanup_and_selected_providers() {
    let (plan, operational) = recorded_calls();
    let applications = ProofApplications::new(&plan, &operational);
    let mut members =
        applications.application_machines("windows_x86_64", false, &[(symbol(6), symbol(7))]);
    members.sort_unstable_by_key(|member| member.arena_index());
    assert_eq!(members, [1, 2, 3, 5, 6, 7].map(symbol));
    assert_eq!(
        applications.application_machines("macos_arm64", false, &[]),
        vec![symbol(9)]
    );
}

#[test]
fn discovery_keeps_selected_scope_first_and_skips_unrelated_scopes() {
    let (mut plan, operational) = recorded_calls();
    let empty = ProofApplications::new(&plan, &operational);
    assert_eq!(empty.scopes("macos_arm64"), ["macos_arm64"]);
    assert!(!empty.needs_scope("windows_x86_64", "macos_arm64"));
    plan.has_obligations = true;
    plan.obligation_owners = plan.symbols.insert_many([symbol(3)]);
    let demands = ProofApplications::new(&plan, &operational);
    assert_eq!(
        demands.scopes("macos_arm64"),
        ["macos_arm64", "windows_x86_64"]
    );
    assert!(demands.needs_scope("windows_x86_64", "macos_arm64"));
    assert!(demands.needs_scope("macos_arm64", "macos_arm64"));
    assert!(!demands.needs_scope("linux_arm64", "macos_arm64"));
}

#[test]
fn repeated_declaration_payloads_contribute_all_formation_dependencies() {
    let mut plan = ProofApplicationPlan::default();
    for (owner, dependencies) in [(1, vec![2]), (2, vec![3]), (2, vec![4]), (4, vec![2, 5])] {
        let dependencies = plan
            .symbols
            .insert_many(dependencies.into_iter().map(symbol));
        plan.declaration_dependencies
            .insert(ProofDeclarationDependencies {
                declaration: symbol(owner),
                dependencies,
                ..Default::default()
            });
    }
    assert_eq!(
        plan.declaration_membership(&[symbol(1)]),
        [2, 3, 4, 5].map(symbol)
    );
}

#[test]
fn retained_metadata_does_not_restore_pruned_seeds_or_provider_bodies() {
    let (mut plan, operational) = recorded_calls();
    let captured_count = plan.machines.len();
    plan.retain_product_machines(|machine| ![symbol(7), symbol(9)].contains(&machine));
    assert_eq!(plan.machines.len(), captured_count);
    let applications = ProofApplications::new(&plan, &operational);
    let mut members =
        applications.application_machines("windows_x86_64", false, &[(symbol(6), symbol(7))]);
    members.sort_unstable_by_key(|member| member.arena_index());
    assert_eq!(members, [1, 2, 3, 5, 6].map(symbol));
    assert!(
        applications
            .application_machines("macos_arm64", false, &[])
            .is_empty()
    );
}
