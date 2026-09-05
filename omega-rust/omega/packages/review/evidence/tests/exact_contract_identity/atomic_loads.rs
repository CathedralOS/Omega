use crate::support::*;

fn atomic_load_review(ordering: &str) -> (CheckedCompilation, CheckedPackageReviewProjection) {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        &format!("pub proposition reviewed(value: &AtomicU32) = value.load({ordering}) == 0u32;\n"),
    );
    package.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
        package_inputs(&package.0),
    )
    .expect("atomic load in a public proposition should check");
    let review = project_checked_package_review(&checked)
        .expect("atomic load should have exact package-review identity");
    (checked, review)
}

#[test]
fn review_projects_each_legal_atomic_load_ordering_and_round_trips_rows() {
    let mut encodings = Vec::new();
    for (ordering, expected) in [
        ("NoOrdering", PackageReviewAtomicLoadOrdering::NoOrdering),
        ("Receive", PackageReviewAtomicLoadOrdering::Receive),
        ("GlobalOrder", PackageReviewAtomicLoadOrdering::GlobalOrder),
    ] {
        let (checked, review) = atomic_load_review(ordering);
        let [proposition] = review.public_propositions() else {
            panic!("one public proposition")
        };
        let PackageReviewPublicPropositionBody::Transparent(PackageReviewContractFact::Expression(
            PackageReviewContractExpression::Binary { left, .. },
        )) = proposition.body()
        else {
            panic!("one transparent atomic-load comparison")
        };
        assert_eq!(
            left.as_ref(),
            &PackageReviewContractExpression::AtomicLoad {
                value: Box::new(PackageReviewContractExpression::Parameter(0)),
                ordering: expected,
            }
        );

        let rows = review.canonical_rows().expect("atomic-load canonical rows");
        for row in &rows {
            let recovered = decode_package_review_canonical_row(
                &encode_package_review_canonical_row(row)
                    .expect("atomic-load recovery envelope should encode"),
            )
            .expect("atomic-load recovery envelope should decode");
            assert_eq!(recovered.canonical_bytes(), row.canonical_bytes());
        }
        let closure = checked
            .dependency_closure()
            .cloned()
            .expect("package-aware compilation retains dependency closure");
        let ledger = ordinary_package_obligation_ledger_from_compiler_rows(closure, &rows)
            .expect("atomic-load rows form an ordinary obligation ledger");
        let recovered = decode_ordinary_package_obligation_ledger(
            &encode_ordinary_package_obligation_ledger(&ledger)
                .expect("atomic-load obligation ledger should encode"),
        )
        .expect("atomic-load obligation ledger should decode");
        assert_eq!(recovered, ledger);
        validate_ordinary_package_obligation_ledger(&recovered, &checked)
            .expect("atomic-load ledger should rederive from checked semantics");

        encodings.push(review.canonical_review_bytes().unwrap());
    }
    encodings.sort();
    encodings.dedup();
    assert_eq!(encodings.len(), 3);

    let (_, relocated) = atomic_load_review("NoOrdering");
    let (_, equivalent) = atomic_load_review("NoOrdering");
    assert_eq!(
        relocated.canonical_review_bytes().unwrap(),
        equivalent.canonical_review_bytes().unwrap(),
        "source relocation must not change atomic-load identity"
    );
}

#[test]
fn atomic_load_review_rejects_non_load_orderings_and_result_tamper() {
    let (mut checked, _) = atomic_load_review("NoOrdering");
    let atomic_expression = checked
        .typed
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| {
            matches!(node, typed_trees::expression::ExpressionNode::Atomic(_)).then_some(expression)
        })
        .expect("atomic load expression");

    use language_core::atomic::{AtomicOrderingPlan, MemoryOrdering};
    for ordering in [
        AtomicOrderingPlan::Store(MemoryOrdering::NoOrdering),
        AtomicOrderingPlan::ReadModifyWrite(MemoryOrdering::NoOrdering),
        AtomicOrderingPlan::Swap(MemoryOrdering::NoOrdering),
        AtomicOrderingPlan::CompareExchange {
            success: MemoryOrdering::NoOrdering,
            failure: MemoryOrdering::NoOrdering,
        },
        AtomicOrderingPlan::CompareExchangeOnce {
            success: MemoryOrdering::NoOrdering,
            failure: MemoryOrdering::NoOrdering,
        },
        AtomicOrderingPlan::Load(MemoryOrdering::Publish),
        AtomicOrderingPlan::Load(MemoryOrdering::ReceivePublish),
    ] {
        let typed_trees::expression::ExpressionNode::Atomic(atomic) = checked
            .typed
            .expression_table
            .expression_mut(atomic_expression)
        else {
            unreachable!()
        };
        atomic.ordering = ordering;
        assert!(project_checked_package_review(&checked).is_err());
    }

    let typed_trees::expression::ExpressionNode::Atomic(atomic) = checked
        .typed
        .expression_table
        .expression_mut(atomic_expression)
    else {
        unreachable!()
    };
    atomic.ordering = AtomicOrderingPlan::Load(MemoryOrdering::NoOrdering);
    atomic.result = atomic.value;
    assert!(
        project_checked_package_review(&checked).is_err(),
        "a load cannot acquire a result carrier after checking"
    );
}
