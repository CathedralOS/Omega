//! Graph controls moved with the former driver-local application traversal.
//! These construct graph records, not source programs or checked judgments.

#[test]
fn published_graph_preserves_nested_bindings_and_declaration_membership() {
    let (mut plan, mut operational) = recorded_calls();
    let nested = operational
        .static_machine_bindings
        .insert_many([StaticMachineCallBinding {
            parameter: symbol(11),
            selected: symbol(4),
            ..Default::default()
        }]);
    operational
        .static_machine_bindings
        .for_each_mut(|_, binding| {
            if binding.parameter == symbol(10) {
                binding.arguments = nested;
            }
        });
    let nested_calls = operational.calls.insert_many([CallOperational {
        static_machine_parameter: symbol(11),
        ..Default::default()
    }]);
    let nested_states = operational.states.insert_many([StateOperational {
        calls: nested_calls,
        ..Default::default()
    }]);
    let mut summaries = operational.machines().to_vec();
    summaries.push(MachineOperational {
        symbol: symbol(3),
        states: nested_states,
        ..Default::default()
    });
    operational.root_machines = operational.machines.insert_many(summaries);
    let dynamic = plan.symbols.insert_many([symbol(8)]);
    plan.machines.for_each_mut(|_, machine| {
        if machine.machine == symbol(3) {
            machine.dynamic_realizations = dynamic;
        }
    });
    let dependencies = plan.symbols.insert_many([symbol(20)]);
    plan.declaration_dependencies
        .insert(ProofDeclarationDependencies {
            declaration: symbol(4),
            dependencies,
            ..Default::default()
        });
    let map = |symbol: SymbolHandle| -> Result<u32, &'static str> {
        Ok(if symbol.is_valid() {
            100 + symbol.arena_index()
        } else {
            0
        })
    };
    let portable_plan = plan.try_map(map).unwrap();
    let portable_calls = ProofCallGraph::capture(
        &operational,
        plan.machines.iter().map(|(_, row)| row.machine),
        &mut { map },
    )
    .unwrap();
    let source = ProofApplications::new(&plan, &operational);
    let portable = semantic_vocabulary::proof_applications::ProofApplications::new(
        &portable_plan,
        &portable_calls,
    );
    let source_members = source.membership("windows_x86_64", false, &[(symbol(6), symbol(7))]);
    let portable_members = portable.membership("windows_x86_64", false, &[(106, 107)]);
    assert_eq!(
        portable_members.0,
        source_members
            .0
            .iter()
            .map(|symbol| map(*symbol).unwrap())
            .collect::<Vec<_>>()
    );
    assert_eq!(portable_members.1, [120]);
    let mut machines = portable_members.0;
    machines.sort_unstable();
    assert_eq!(machines, [101, 102, 103, 104, 105, 106, 107, 108]);
    assert_eq!(portable.membership("macos_arm64", false, &[]).0, [109]);
    assert_eq!(
        portable
            .membership("windows_x86_64", false, &[])
            .0
            .contains(&107),
        false
    );
    // Missing source summaries do not become empty summaries during capture.
    assert!(
        !portable_calls
            .machines
            .iter()
            .any(|(_, row)| row.machine == 108)
    );
}

use super::*;
use flow_effects::{
    CallOperational, CleanupDependency, MachineOperational, StateOperational,
    StaticMachineCallBinding,
};
use semantic_vocabulary::proof_applications::ProofCallGraph;

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
    let mut members = applications
        .membership("windows_x86_64", false, &[(symbol(6), symbol(7))])
        .0;
    members.sort_unstable_by_key(|member| member.arena_index());
    assert_eq!(members, [1, 2, 3, 5, 6, 7].map(symbol));
    assert_eq!(
        applications.membership("macos_arm64", false, &[]).0,
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
fn retained_metadata_does_not_restore_pruned_seeds_or_provider_bodies() {
    let (mut plan, operational) = recorded_calls();
    let captured_count = plan.machines.len();
    plan.retain_product_machines(|machine| ![symbol(7), symbol(9)].contains(&machine));
    assert_eq!(plan.machines.len(), captured_count);
    let applications = ProofApplications::new(&plan, &operational);
    let mut members = applications
        .membership("windows_x86_64", false, &[(symbol(6), symbol(7))])
        .0;
    members.sort_unstable_by_key(|member| member.arena_index());
    assert_eq!(members, [1, 2, 3, 5, 6].map(symbol));
    assert!(
        applications
            .membership("macos_arm64", false, &[])
            .0
            .is_empty()
    );
}
