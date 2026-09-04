use crate::support::*;

#[test]
fn review_projects_named_operator_calls_in_public_propositions() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Token [copy] { value: u64; }
pub operator Token::ordered(left: Token, right: Token) -> bool;
pub proposition reviewed(left: Token, right: Token) = Token::ordered(left, right);
"#,
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
    .expect("named operator call in a public proposition should check");
    let review = project_checked_package_review(&checked)
        .expect("named operator call should have exact package-review identity");
    let reviewed = review
        .public_propositions()
        .iter()
        .find(|proposition| proposition.identity().path() == "reviewed")
        .expect("reviewed proposition row");
    let PackageReviewPublicPropositionBody::Transparent(PackageReviewContractFact::Expression(
        PackageReviewContractExpression::Call {
            receiver,
            target,
            static_arguments,
            arguments,
            ..
        },
    )) = reviewed.body()
    else {
        panic!("one named operator call")
    };
    assert_eq!(
        target.nominal().expect("nominal operator target").path(),
        "Token::ordered"
    );
    assert!(
        receiver.is_none(),
        "a static namespace is not a value receiver"
    );
    assert!(static_arguments.is_empty());
    assert_eq!(
        arguments,
        &[
            PackageReviewContractExpression::Parameter(0),
            PackageReviewContractExpression::Parameter(1),
        ]
    );
}

#[test]
fn named_operator_review_rejects_post_check_target_drift() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Token [copy] { value: u64; }
pub operator Token::ordered(left: Token, right: Token) -> bool;
pub operator Token::reversed(left: Token, right: Token) -> bool;
pub proposition reviewed(left: Token, right: Token) = Token::ordered(left, right);
"#,
    );
    package.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let mut checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
        package_inputs(&package.0),
    )
    .expect("named operator target-tamper fixture should check");
    let call = checked
        .typed
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| {
            matches!(
                node,
                psi_typed_trees::expression::ExpressionNode::Call(call)
                    if call.target.as_str() == "ordered"
            )
            .then_some(expression)
        })
        .expect("ordered named operator call");
    let psi_typed_trees::expression::ExpressionNode::Call(call) =
        checked.typed.expression_table.expression_mut(call)
    else {
        unreachable!()
    };
    call.target = psi_typed_trees::name::Identifier::generated_static("reversed");

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("post-check named operator target drift must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("target disagrees with its exact checked call-selection row")
    }));
}
