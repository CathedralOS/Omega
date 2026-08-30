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
        r#"target windows_x86_64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
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
        psi_language_semantics::DomainPredicateBody::Present
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
