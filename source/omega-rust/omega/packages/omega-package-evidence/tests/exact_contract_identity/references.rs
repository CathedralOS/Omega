use crate::support::*;

fn reference_review(access: &str) -> CheckedPackageReviewProjection {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        &format!(
            "pub proposition observes(value: &{access}i32);\n\
             pub proposition reviewed(value: i32) = observes(&{access}value);\n"
        ),
    );
    package.write(
        "build.omg",
        "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("reference-bearing proposition should check");
    project_checked_package_review(&checked)
        .expect("reference formation should have exact package-review identity")
}

#[test]
fn review_projects_exact_reference_formation_in_public_propositions() {
    let shared = reference_review("");
    let mutable = reference_review("mut ");

    let shared_reviewed = shared
        .public_propositions()
        .iter()
        .find(|proposition| proposition.identity().path() == "reviewed")
        .expect("reviewed public proposition");
    let PackageReviewPublicPropositionBody::Transparent(PackageReviewContractFact::Proposition(
        application,
    )) = shared_reviewed.body()
    else {
        panic!("transparent proposition application")
    };
    assert_eq!(
        application.arguments(),
        &[PackageReviewContractExpression::Parameter(0)]
    );

    let mutable_reviewed = mutable
        .public_propositions()
        .iter()
        .find(|proposition| proposition.identity().path() == "reviewed")
        .expect("mutable reviewed public proposition");
    let PackageReviewPublicPropositionBody::Transparent(PackageReviewContractFact::Proposition(
        application,
    )) = mutable_reviewed.body()
    else {
        panic!("mutable transparent proposition application")
    };
    assert_eq!(
        application.arguments(),
        &[PackageReviewContractExpression::Reference {
            access: PackageReviewReferenceAccess::Mutable,
            target: Box::new(PackageReviewContractExpression::Parameter(0)),
        }]
    );
    assert_ne!(
        shared.canonical_review_bytes().unwrap(),
        mutable.canonical_review_bytes().unwrap(),
        "reference access is part of package-review identity",
    );
}

#[test]
fn reference_review_rejects_access_tamper_after_checking() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        "pub proposition observes(value: &mut i32);\n\
         pub proposition reviewed(value: i32) = observes(&mut value);\n",
    );
    package.write(
        "build.omg",
        "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let mut checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("reference-tamper fixture should check");
    let reviewed = checked
        .propositions()
        .iter()
        .find(|proposition| proposition.name.as_str() == "reviewed")
        .expect("reviewed proposition");
    let psi_typed_trees::proposition::PropositionBody::Transparent {
        proposition: psi_typed_trees::proposition::PropositionFormula::Application(application),
    } = &reviewed.body
    else {
        panic!("transparent proposition application")
    };
    let [borrow] = checked
        .expression_table
        .expression_handles(application.arguments)
    else {
        panic!("one reference argument")
    };
    let borrow = *borrow;
    let psi_typed_trees::expression::ExpressionNode::Borrow(borrow) =
        checked.typed.expression_table.expression_mut(borrow)
    else {
        unreachable!()
    };
    borrow.access = psi_language_core::ReferenceAccess::Shared;

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("review must reject reference access changed after checking");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not match its proposition parameter type")
    }));
}

#[test]
fn contract_call_reference_review_rejects_access_tamper_after_checking() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        "pub machine observes(value: &mut i32) -> bool { true }\n\
         pub proposition reviewed(value: i32) = observes(&mut value);\n",
    );
    package.write(
        "build.omg",
        "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let mut checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("contract-call reference-tamper fixture should check");
    let borrow = checked
        .typed
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| {
            matches!(node, psi_typed_trees::expression::ExpressionNode::Borrow(_))
                .then_some(expression)
        })
        .expect("contract-call borrow expression");
    let psi_typed_trees::expression::ExpressionNode::Borrow(borrow) =
        checked.typed.expression_table.expression_mut(borrow)
    else {
        unreachable!()
    };
    borrow.access = psi_language_core::ReferenceAccess::Shared;

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("review must reject contract-call reference access changed after checking");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not match its contract-call parameter type")
    }));
}
