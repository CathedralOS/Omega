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
        callable: TerminalCallable::Boundary(BoundaryMachineId::new(1).unwrap()),
    });
    for ordinal in 2..=3 {
        scope.callables.insert(ProofCallableAssociation::Callable {
            declaration: ProofDeclarationId(ordinal),
            callable: TerminalCallable::Machine(MachineId::new(u64::from(ordinal)).unwrap()),
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
fn scalar_condition_scope_preserves_all_selected_product_bodies() {
    let (scope, conditions) = fixture();
    let boundary = BoundaryMachineId::new(1).unwrap();
    for body in [2, 3] {
        let selected = ScalarConditionSelection::select(
            &scope,
            &conditions,
            "macos_arm64",
            false,
            [(boundary, MachineId::new(body).unwrap())],
            &[],
        )
        .unwrap();
        assert_eq!(selected.ranges(), [0, 1, 2, 3, 5]);
        assert_eq!(selected.predicates(), [0, 1, 2, 5]);
        assert_eq!(
            selected.applications().unwrap(),
            [ScalarApplicationId::new(7).unwrap()]
        );
    }
    let none = ScalarConditionSelection::select(&scope, &conditions, "macos_arm64", false, [], &[])
        .unwrap();
    assert_eq!(none.ranges(), [0, 1, 2, 3, 5]);
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
    assert_eq!(secondary.ranges(), [0, 1, 2, 3, 5]);
    assert_eq!(secondary.predicates(), [0, 1, 2, 5]);
    let primary = ScalarConditionSelection::select(
        &scope,
        &conditions,
        "windows_x86_64",
        true,
        [],
        &[pending],
    )
    .unwrap();
    // The same ordinary entry stays selected; declaration 5 owes
    // address formation, but never an unrelated predicate or fixed range.
    assert_eq!(primary.ranges(), [0, 1, 2, 3, 4, 5, 6]);
    assert_eq!(primary.predicates(), [0, 1, 2, 5, 6]);
}

#[test]
fn target_names_do_not_select_source_declarations() {
    let (scope, conditions) = fixture();
    for target in ["macos_arm64", "windows_x86_64", "linux_arm64"] {
        let selected =
            ScalarConditionSelection::select(&scope, &conditions, target, false, [], &[]).unwrap();
        assert_eq!(selected.predicates(), [0, 1, 2, 5]);
        assert_eq!(selected.ranges(), [0, 1, 2, 3, 5]);
    }
}
