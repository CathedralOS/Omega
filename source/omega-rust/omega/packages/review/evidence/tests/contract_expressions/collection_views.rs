use crate::support::*;

fn compile_collection_view(expression: &str) -> CheckedCompilation {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        &format!(
            r#"pub machine accepts_view(items: [u8; 4])
requires valid_utf8({expression})
{{}}
"#,
        ),
    );
    package.write(
        "build.omg",
        r#"target windows_x86_64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
        package_inputs(&package.0),
    )
    .expect("public collection-view contract should check")
}

fn projected_collection_view(
    checked: &CheckedCompilation,
) -> (
    PackageReviewContractExpression,
    PackageReviewCollectionViewOperation,
) {
    let review = project_checked_package_review(checked)
        .expect("collection-view intrinsic should have exact review identity");
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "accepts_view")
        .expect("public callable row");
    let [contract] = callable.contracts() else {
        panic!("one collection-view contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Call {
        arguments,
        ..
    }) = contract.fact()
    else {
        panic!("byte predicate fact")
    };
    let [view] = arguments.as_slice() else {
        panic!("one byte predicate argument")
    };
    let PackageReviewContractExpression::Call { target, .. } = view else {
        panic!("collection-view call")
    };
    (
        view.clone(),
        target
            .collection_view()
            .expect("compiler-owned collection-view target"),
    )
}

fn collection_view_expression(
    checked: &CheckedCompilation,
) -> psi_typed_trees::expression::ExpressionHandle {
    let matching = checked
        .facts
        .intrinsic_calls
        .iter()
        .filter_map(|fact| {
            matches!(
                fact.intrinsic,
                psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic::CollectionView(_)
            )
            .then_some(fact.expression)
        })
        .collect::<Vec<_>>();
    let [expression] = matching.as_slice() else {
        panic!("one checked collection-view fact")
    };
    *expression
}

#[test]
fn review_projects_each_collection_view_as_closed_intrinsic_identity() {
    for (expression, expected) in [
        (
            "items.as_slice()",
            PackageReviewCollectionViewOperation::SharedSlice,
        ),
        (
            "items.as_mut_slice()",
            PackageReviewCollectionViewOperation::MutableSlice,
        ),
        (
            "\"hello\".as_view()",
            PackageReviewCollectionViewOperation::TextView,
        ),
        (
            "\"hello\".bytes()",
            PackageReviewCollectionViewOperation::Bytes,
        ),
    ] {
        let checked = compile_collection_view(expression);
        let (view, operation) = projected_collection_view(&checked);
        assert_eq!(operation, expected);
        let PackageReviewContractExpression::Call {
            receiver,
            static_arguments,
            arguments,
            ..
        } = &view
        else {
            unreachable!()
        };
        assert!(receiver.is_some());
        assert!(static_arguments.is_empty());
        assert!(arguments.is_empty());
        project_checked_package_review(&checked)
            .unwrap()
            .canonical_review_bytes()
            .expect("collection-view review must be canonically encodable");
    }
}

#[test]
fn collection_view_review_rejects_checked_identity_and_custody_tamper() {
    let assert_rejects = |checked: &CheckedCompilation, expected: &str| {
        let diagnostics = project_checked_package_review(checked)
            .expect_err("tampered collection-view identity must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing `{expected}` in {diagnostics:#?}"
        );
    };

    let mut missing = compile_collection_view("items.as_slice()");
    let expression = collection_view_expression(&missing);
    missing
        .facts
        .intrinsic_calls
        .retain(|fact| fact.expression != expression);
    assert_rejects(&missing, "0 retained checked intrinsic facts");

    let mut duplicate = compile_collection_view("items.as_slice()");
    let expression = collection_view_expression(&duplicate);
    let fact = duplicate
        .facts
        .intrinsic_calls
        .iter()
        .find(|fact| fact.expression == expression)
        .copied()
        .expect("collection-view fact");
    duplicate.facts.intrinsic_calls.push(fact);
    assert_rejects(&duplicate, "2 retained checked intrinsic facts");

    let mut redirected = compile_collection_view("items.as_slice()");
    let expression = collection_view_expression(&redirected);
    redirected
        .facts
        .intrinsic_calls
        .iter_mut()
        .find(|fact| fact.expression == expression)
        .expect("collection-view fact")
        .intrinsic = psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic::CollectionView(
        psi_language_semantics::declaration_selection::CollectionViewOperation::MutableSlice,
    );
    assert_rejects(
        &redirected,
        "disagrees with its exact checked intrinsic identity",
    );

    let mut stale_call = compile_collection_view("items.as_slice()");
    let expression = collection_view_expression(&stale_call);
    let psi_typed_trees::expression::ExpressionNode::Call(call) =
        stale_call.typed.expression_table.expression_mut(expression)
    else {
        panic!("collection-view call expression")
    };
    call.target = psi_typed_trees::name::Identifier::generated_static("as_mut_slice");
    assert_rejects(
        &stale_call,
        "disagrees with its exact checked intrinsic identity",
    );

    let mut duplicate_selection = compile_collection_view("items.as_slice()");
    let expression = collection_view_expression(&duplicate_selection);
    let byte_predicate_occurrence = duplicate_selection
        .typed
        .expression_table
        .iter_expressions()
        .find_map(|(candidate, node)| {
            let psi_typed_trees::expression::ExpressionNode::Call(call) = node else {
                return None;
            };
            (call.target.as_str() == "valid_utf8")
                .then(|| {
                    duplicate_selection
                        .typed
                        .expression_table
                        .authored_selection_occurrences(candidate)
                        .next()
                })
                .flatten()
        })
        .expect("byte-predicate call selection");
    duplicate_selection
        .typed
        .expression_table
        .attach_authored_selection_occurrences(expression, [byte_predicate_occurrence]);
    assert_rejects(&duplicate_selection, "2 exact checked call-selection rows");
}

#[test]
fn package_callable_with_collection_view_spelling_remains_nominal() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Wrapper {}
pub machine as_slice(value: &Wrapper) -> bool { true }
pub proposition calls_package(value: &Wrapper) = as_slice(value);
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x86_64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
        package_inputs(&package.0),
    )
    .expect("package collection-view lookalike should check");
    let review = project_checked_package_review(&checked)
        .expect("package collection-view lookalike should project nominally");
    let proposition = review
        .public_propositions()
        .iter()
        .find(|proposition| proposition.identity().path() == "calls_package")
        .expect("lookalike proposition row");
    let PackageReviewPublicPropositionBody::Transparent(PackageReviewContractFact::Expression(
        PackageReviewContractExpression::Call { target, .. },
    )) = proposition.body()
    else {
        panic!("lookalike proposition call")
    };
    let nominal = target
        .nominal()
        .expect("package-authored lookalike must remain nominal");
    assert_eq!(nominal.path(), "as_slice::entry");
    assert_eq!(
        nominal.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
}
