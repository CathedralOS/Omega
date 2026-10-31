use super::*;
use arena::Arena;
use numerics::{bignum::BigInt, scalar_predicates::ScalarValue};
use semantic_vocabulary::proof_applications::{
    ProofApplicationCall, ProofApplicationMachine, ProofCallMachine, ProofDeclarationDependencies,
};
use terminal_psi::{
    ProofCallableAssociation, ProofDeclarationId, ProofScopeDeclaration, ScalarPredicateNode,
    TerminalIdentityIndex,
};

fn fixture() -> (ProofApplicationScope, ScalarConditions) {
    let mut scope = ProofApplicationScope::default();
    for _ in 1..=7 {
        scope
            .declarations
            .insert(ProofScopeDeclaration::Nominal(TerminalIdentityIndex(1)));
    }
    for ordinal in 1..=3 {
        scope.plan.machines.insert(ProofApplicationMachine {
            machine: ProofDeclarationId(ordinal),
            source_target: (ordinal == 1).then(|| "macos_arm64".into()),
            product_selected: true,
            ..Default::default()
        });
    }
    let call = scope.calls.calls.insert_many([ProofApplicationCall {
        callee: ProofDeclarationId(6),
        ..Default::default()
    }]);
    scope.calls.machines.insert(ProofCallMachine {
        machine: ProofDeclarationId(1),
        calls: call,
        ..Default::default()
    });
    scope.plan.declarations = scope
        .plan
        .symbols
        .insert_many([ProofDeclarationId(4), ProofDeclarationId(5)]);
    let dependency = scope.plan.symbols.insert_many([ProofDeclarationId(4)]);
    scope
        .plan
        .declaration_dependencies
        .insert(ProofDeclarationDependencies {
            declaration: ProofDeclarationId(2),
            dependencies: dependency,
            ..Default::default()
        });
    scope.callables.insert(ProofCallableAssociation::Callable {
        declaration: ProofDeclarationId(6),
        callable: TargetMachineCallable::Boundary(BoundaryMachineId::new(1).unwrap()),
    });
    for ordinal in 2..=3 {
        scope.callables.insert(ProofCallableAssociation::Callable {
            declaration: ProofDeclarationId(ordinal),
            callable: TargetMachineCallable::Machine(MachineId::new(u64::from(ordinal)).unwrap()),
        });
        scope
            .plan
            .provider_candidates
            .insert((ProofDeclarationId(6), ProofDeclarationId(ordinal)));
    }
    let mut conditions = ScalarConditions::default();
    let pending = ScalarApplicationId::new(7).unwrap();
    for subject in (1..=5)
        .map(|id| ScalarConditionSubject::Declaration(ProofDeclarationId(id)))
        .chain([
            ScalarConditionSubject::Module,
            ScalarConditionSubject::Application(pending),
        ])
    {
        conditions
            .retain_range(
                subject,
                ScalarRangeRequirement::AddressModel,
                vec![IntegerBoundStep::Observation(
                    ScalarRangeObservation::Application(pending),
                )],
            )
            .unwrap();
        let mut nodes = Arena::new();
        let root = nodes.insert(ScalarPredicateNode(PredicateNode::Value(
            ScalarValue::Boolean(true),
        )));
        conditions
            .retain_predicate(subject, "scope test".into(), nodes, root)
            .unwrap();
    }
    conditions
        .retain_range(
            ScalarConditionSubject::Declaration(ProofDeclarationId(4)),
            ScalarRangeRequirement::Fixed {
                minimum: BigInt::from_u64(0),
                maximum: BigInt::from_u64(255),
            },
            vec![IntegerBoundStep::Bounds {
                low: BigInt::from_u64(0),
                high: BigInt::from_u64(0),
            }],
        )
        .unwrap();
    (scope, conditions)
}

#[test]
fn scalar_condition_scope_uses_selected_edges_not_available_candidates() {
    let (scope, conditions) = fixture();
    let boundary = BoundaryMachineId::new(1).unwrap();
    for (body, ranges, predicates) in [
        (2, vec![0, 1, 3, 5], vec![0, 1, 5]),
        (3, vec![0, 2, 5], vec![0, 2, 5]),
    ] {
        let selected = ScalarConditionSelection::select(
            &scope,
            &conditions,
            "macos_arm64",
            false,
            [(boundary, MachineId::new(body).unwrap())],
            &[],
        )
        .unwrap();
        assert_eq!(selected.ranges(), ranges);
        assert_eq!(selected.predicates(), predicates);
        assert_eq!(
            selected.applications().unwrap(),
            [ScalarApplicationId::new(7).unwrap()]
        );
    }
    let none = ScalarConditionSelection::select(&scope, &conditions, "macos_arm64", false, [], &[])
        .unwrap();
    assert_eq!(none.ranges(), [0, 5]);
    assert!(
        ScalarConditionSelection::select(
            &scope,
            &conditions,
            "macos_arm64",
            false,
            [(boundary, MachineId::new(99).unwrap())],
            &[]
        )
        .is_err()
    );
}

#[test]
fn scalar_condition_scope_keeps_primary_formation_and_explicit_entry_demands_separate() {
    let (scope, conditions) = fixture();
    let pending = ScalarApplicationId::new(7).unwrap();
    let secondary =
        ScalarConditionSelection::select(&scope, &conditions, "windows_x86_64", false, [], &[])
            .unwrap();
    assert_eq!(secondary.ranges(), [5]);
    assert_eq!(secondary.predicates(), [5]);
    let primary = ScalarConditionSelection::select(
        &scope,
        &conditions,
        "windows_x86_64",
        true,
        [],
        &[pending],
    )
    .unwrap();
    // The untargeted selected machines are primary seeds; declaration 5 owes
    // address formation, but never an unrelated predicate or fixed range.
    assert_eq!(primary.ranges(), [1, 2, 3, 4, 5, 6]);
    assert_eq!(primary.predicates(), [1, 2, 5, 6]);
}

#[test]
fn scalar_condition_scope_selects_family_members_without_rooting_other_targets() {
    use semantic_vocabulary::proof_applications::ProofApplicationTargetMember;
    let (mut scope, conditions) = fixture();
    let members = scope.plan.target_members.insert_many([
        ProofApplicationTargetMember {
            target: "macos_arm64".into(),
            machine: ProofDeclarationId(2),
        },
        ProofApplicationTargetMember {
            target: "windows_x86_64".into(),
            machine: ProofDeclarationId(3),
        },
    ]);
    scope.plan.machines.for_each_mut(|_, row| {
        row.source_target = None;
        if row.machine == ProofDeclarationId(1) {
            row.family_interface = true;
            row.family_members = members;
        } else {
            let target = if row.machine == ProofDeclarationId(2) {
                "macos_arm64"
            } else {
                "windows_x86_64"
            };
            row.source_target = Some(target.into());
            row.authored_target = Some(target.into());
        }
    });
    for (target, predicate) in [("macos_arm64", 1), ("windows_x86_64", 2)] {
        let selected =
            ScalarConditionSelection::select(&scope, &conditions, target, true, [], &[]).unwrap();
        assert_eq!(selected.predicates(), [predicate, 5]);
        assert!(
            !selected.predicates().contains(&0),
            "family head is not a selected body"
        );
    }
    let unavailable =
        ScalarConditionSelection::select(&scope, &conditions, "linux_arm64", false, [], &[])
            .unwrap();
    assert_eq!(unavailable.predicates(), [5]);
    assert_eq!(unavailable.ranges(), [5]);
}
