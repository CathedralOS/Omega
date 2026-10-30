use super::*;

#[test]
fn family_binder_rebasing_keeps_distinct_static_applications() {
    let mut plan = ProofApplicationPlan::<u32>::default();
    machine(&mut plan, 1, Some("macos_arm64"));
    let interface_binders = plan.symbols.insert_many([10]);
    let member_binders = plan.symbols.insert_many([11]);
    let members = plan
        .target_members
        .insert_many([ProofApplicationTargetMember {
            target: "macos_arm64".into(),
            machine: 3,
        }]);
    plan.machines.insert(ProofApplicationMachine {
        machine: 2,
        binders: interface_binders,
        family_interface: true,
        family_members: members,
        product_selected: true,
        ..Default::default()
    });
    plan.machines.insert(ProofApplicationMachine {
        machine: 3,
        binders: member_binders,
        authored_target: Some("macos_arm64".into()),
        // Not an authored root: it is reached through the exact family application.
        product_selected: true,
        ..Default::default()
    });
    machine(&mut plan, 4, None);
    machine(&mut plan, 5, None);
    let mut graph = ProofCallGraph::<u32>::default();
    let first = graph.bindings.insert_many([ProofStaticBinding {
        parameter: 10,
        selected: 4,
        ..Default::default()
    }]);
    let second = graph.bindings.insert_many([ProofStaticBinding {
        parameter: 10,
        selected: 5,
        ..Default::default()
    }]);
    let calls = graph.calls.insert_many([
        ProofApplicationCall {
            callee: 2,
            arguments: first,
            ..Default::default()
        },
        ProofApplicationCall {
            callee: 2,
            arguments: second,
            ..Default::default()
        },
    ]);
    graph.machines.insert(ProofCallMachine {
        machine: 1,
        calls,
        ..Default::default()
    });
    let calls = graph.calls.insert_many([ProofApplicationCall {
        binder: 11,
        ..Default::default()
    }]);
    graph.machines.insert(ProofCallMachine {
        machine: 3,
        calls,
        ..Default::default()
    });
    let mut reached = ProofApplications::new(&plan, &graph)
        .membership("macos_arm64", false, &[], None)
        .0;
    reached.sort_unstable();
    assert_eq!(reached, [1, 3, 4, 5]);
    assert!(
        ProofApplications::new(&plan, &graph)
            .membership("windows_x86_64", false, &[], None)
            .0
            .is_empty()
    );
}

fn symbol(ordinal: u32) -> u32 {
    ordinal
}

fn machine(plan: &mut ProofApplicationPlan<u32>, ordinal: u32, scope: Option<&str>) {
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
