use super::*;

#[test]
fn ordinary_binders_keep_distinct_static_applications() {
    let mut plan = ProofApplicationPlan::<u32>::default();
    for ordinal in [1, 2, 4, 5] {
        plan.machines.insert(ProofApplicationMachine {
            machine: ordinal,
            product_selected: true,
            ..Default::default()
        });
    }
    let mut graph = ProofCallGraph::<u32>::default();
    let mut calls = Vec::new();
    for selected in [4, 5] {
        let arguments = graph.bindings.insert_many([ProofStaticBinding {
            parameter: 10,
            selected,
            ..Default::default()
        }]);
        calls.push(ProofApplicationCall {
            callee: 2,
            arguments,
            ..Default::default()
        });
    }
    let calls = graph.calls.insert_many(calls);
    graph.machines.insert(ProofCallMachine {
        machine: 1,
        calls,
        ..Default::default()
    });
    let calls = graph.calls.insert_many([ProofApplicationCall {
        binder: 10,
        ..Default::default()
    }]);
    graph.machines.insert(ProofCallMachine {
        machine: 2,
        calls,
        ..Default::default()
    });
    let mut reached = ProofApplications::new(&plan, &graph)
        .reachability(&[], Some(&[1]))
        .machines;
    reached.sort_unstable();
    assert_eq!(reached, [1, 2, 4, 5]);
}

fn symbol(ordinal: u32) -> u32 {
    ordinal
}

#[test]
fn retained_ordinary_declarations_are_not_implicit_roots() {
    let mut plan = ProofApplicationPlan::<u32>::default();
    for ordinal in [1, 2] {
        plan.machines.insert(ProofApplicationMachine {
            machine: ordinal,
            product_selected: ordinal == 1,
            ..Default::default()
        });
    }
    let graph = ProofCallGraph::<u32>::default();
    assert_eq!(ProofApplications::new(&plan, &graph).membership(&[]).0, [1]);
    plan.retain_product_machines(|_| false);
    assert!(
        ProofApplications::new(&plan, &graph)
            .membership(&[])
            .0
            .is_empty()
    );
    assert_eq!(plan.machines.len(), 2);
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
