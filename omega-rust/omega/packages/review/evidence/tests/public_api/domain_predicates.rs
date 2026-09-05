use crate::support::*;

#[test]
fn review_projects_public_domain_predicates_from_exact_checked_rows() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Packet { value: u32; }
pub domain Packet::Ready
    requires self.value == 0;
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
        package_inputs(&package.0),
    )
    .expect("public domain fact fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("review should project the checked public domain predicate");
    let [domain] = review.public_domains() else {
        panic!("one public domain row")
    };
    assert_eq!(
        domain.predicate_body(),
        language_semantics::DomainPredicateBody::Present
    );
    let [
        PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
            meaning,
            operator,
            left,
            right,
        }),
    ] = domain.predicate_facts()
    else {
        panic!("one binary domain predicate fact")
    };
    assert_eq!(meaning, &PackageReviewContractOperatorMeaning::Builtin);
    assert_eq!(*operator, PackageReviewContractBinaryOperator::Equal);
    let PackageReviewContractExpression::Member {
        receiver,
        member,
        case_variant,
    } = left.as_ref()
    else {
        panic!("domain-subject member path")
    };
    assert_eq!(
        receiver.as_ref(),
        &PackageReviewContractExpression::DomainSubject
    );
    assert_eq!(member.path(), "Packet::value");
    assert!(case_variant.is_none());
    assert_eq!(
        right.as_ref(),
        &PackageReviewContractExpression::Integer("0".to_owned())
    );
}

#[test]
fn review_projects_exact_owner_nominal_calls_in_public_domain_predicates() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Packet [copy] { value: u32; }

pub machine Packet::attached_ready(&self) -> bool { true }
pub machine Packet::qualified_ready(value: Packet) -> bool { true }

pub domain Packet::Ready
requires
    self.attached_ready(),
    Packet::qualified_ready(self);
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let mut checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
        package_inputs(&package.0),
    )
    .expect("exact-owner domain calls should check");
    let review = project_checked_package_review(&checked)
        .expect("exact-owner domain calls should retain package review custody");
    let [domain] = review.public_domains() else {
        panic!("one public domain row")
    };

    let calls = domain
        .predicate_facts()
        .iter()
        .map(|fact| {
            let PackageReviewContractFact::Expression(PackageReviewContractExpression::Call {
                receiver,
                target,
                static_arguments,
                evidence_arguments,
                arguments,
            }) = fact
            else {
                panic!("one exact nominal call per domain predicate")
            };
            assert!(static_arguments.is_empty());
            assert!(evidence_arguments.is_empty());
            (
                target.nominal().expect("ordinary nominal target").path(),
                receiver.as_deref(),
                arguments.as_slice(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    assert!(calls.iter().any(|(target, receiver, arguments)| {
        target.contains("attached_ready")
            && *receiver == Some(&PackageReviewContractExpression::DomainSubject)
            && arguments.is_empty()
    }));
    assert!(calls.iter().any(|(target, receiver, arguments)| {
        target.contains("qualified_ready")
            && receiver.is_none()
            && *arguments == [PackageReviewContractExpression::DomainSubject]
    }));

    let qualified_target = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .find(|state| state.name.as_str() == "qualified_ready")
        .expect("qualified target state")
        .symbol;
    let attached_call = checked
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| {
            let typed_trees::expression::ExpressionNode::Call(call) = node else {
                return None;
            };
            (call.target.as_str() == "attached_ready").then_some(expression)
        })
        .expect("attached domain call");
    let typed_trees::expression::ExpressionNode::Call(call) =
        checked.typed.expression_table.expression_mut(attached_call)
    else {
        panic!("attached domain call expression")
    };
    call.target_symbol = qualified_target;
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("stored target must not override exact owner derivation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("retained target disagrees with its exact checked owner derivation")
    }));
}
