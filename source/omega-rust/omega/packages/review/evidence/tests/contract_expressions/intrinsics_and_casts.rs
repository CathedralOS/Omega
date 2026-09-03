use crate::support::*;

#[test]
fn review_projects_collection_length_as_an_exact_compiler_intrinsic() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        "pub proposition non_empty(items: &[u8]) = items.len > 0;\n",
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
    .expect("public collection-length proposition should check");
    let length_selection = checked
        .authored_declaration_selections()
        .iter()
        .find(|selection| {
            selection.kind()
                == psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::MemberAccess
                && selection.target()
                    == psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Intrinsic(
                        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic::CollectionLength,
                    )
        })
        .expect("checked contract must retain its exact collection-length selection");
    assert_eq!(
        length_selection.exposure(),
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
    );

    let review = project_checked_package_review(&checked)
        .expect("collection-length intrinsic should have exact review identity");
    let proposition = review
        .public_propositions()
        .iter()
        .find(|proposition| proposition.identity().path() == "non_empty")
        .expect("public proposition row");
    let PackageReviewPublicPropositionBody::Transparent(PackageReviewContractFact::Expression(
        PackageReviewContractExpression::Binary { left, .. },
    )) = proposition.body()
    else {
        panic!("binary transparent proposition body")
    };
    assert_eq!(
        left.as_ref(),
        &PackageReviewContractExpression::CollectionLength {
            collection: Box::new(PackageReviewContractExpression::Parameter(0)),
        }
    );
    review
        .canonical_review_bytes()
        .expect("collection-length review must be canonically encodable");
}

#[test]
fn review_rejoins_unary_contract_operator_to_its_exact_compiler_intrinsic() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        "pub proposition inverted(value: u8, expected: u8) = ~value == expected;\n",
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
    .expect("public unary proposition should check");
    let inverted = checked
        .propositions()
        .iter()
        .find(|proposition| proposition.name.as_str() == "inverted")
        .expect("checked public proposition declaration");
    let psi_typed_trees::proposition::PropositionBody::Transparent {
        proposition:
            psi_typed_trees::proposition::PropositionFormula::BooleanExpression(root_expression),
    } = inverted.body
    else {
        panic!("inverted must retain its transparent boolean formula")
    };
    let psi_typed_trees::expression::ExpressionNode::Binary(binary) =
        checked.expression_table.expression(root_expression)
    else {
        panic!("inverted formula must retain its equality root")
    };
    let unary_expression = binary.left;
    assert!(matches!(
        checked.expression_table.expression(unary_expression),
        psi_typed_trees::expression::ExpressionNode::Unary(_)
    ));
    let unary_occurrences = checked
        .expression_table
        .authored_selection_occurrences(unary_expression)
        .collect::<Vec<_>>();
    let [unary_occurrence] = unary_occurrences.as_slice() else {
        panic!("unary contract must retain one exact authored selection")
    };
    let unary_selection = checked
        .authored_declaration_selections()
        .get(*unary_occurrence)
        .expect("unary occurrence must rejoin its checked selection");
    assert_eq!(
        unary_selection.kind(),
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Operator
    );
    assert_eq!(
        unary_selection.target(),
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Intrinsic(
            psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
        )
    );
    assert_eq!(
        unary_selection.exposure(),
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
    );

    let review = project_checked_package_review(&checked)
        .expect("unary compiler intrinsic should rejoin package review");
    let proposition = review
        .public_propositions()
        .iter()
        .find(|proposition| proposition.identity().path() == "inverted")
        .expect("public proposition row");
    assert_eq!(
        proposition.body(),
        &PackageReviewPublicPropositionBody::Transparent(PackageReviewContractFact::Expression(
            PackageReviewContractExpression::Binary {
                meaning: PackageReviewContractOperatorMeaning::Builtin,
                operator: PackageReviewContractBinaryOperator::Equal,
                left: Box::new(PackageReviewContractExpression::Unary {
                    operator: PackageReviewContractUnaryOperator::BitwiseNot,
                    operand: Box::new(PackageReviewContractExpression::Parameter(0)),
                }),
                right: Box::new(PackageReviewContractExpression::Parameter(1)),
            },
        ))
    );
    review
        .canonical_review_bytes()
        .expect("unary compiler intrinsic must remain canonically encodable");
}

#[test]
fn package_field_named_len_remains_a_nominal_member_in_review() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Buffer { len: u64; }
pub machine consume(buffer: Buffer)
requires buffer.len > 0
{ }
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
    .expect("same-spelled package field contract should check");
    let review = project_checked_package_review(&checked)
        .expect("same-spelled package field should retain nominal review identity");
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "consume")
        .expect("public callable row");
    let [contract] = callable.contracts() else {
        panic!("one public callable contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        left, ..
    }) = contract.fact()
    else {
        panic!("binary public callable contract")
    };
    let PackageReviewContractExpression::Member {
        receiver, member, ..
    } = left.as_ref()
    else {
        panic!("package field must remain a nominal member")
    };
    assert_eq!(
        receiver.as_ref(),
        &PackageReviewContractExpression::Parameter(0)
    );
    assert_eq!(member.path(), "Buffer::len");
    assert_eq!(
        member.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
}

#[test]
fn review_projects_contract_casts_without_diagnostic_spelling() {
    let Some(target) = host_target_name() else {
        return;
    };
    let u16_cast = TempPackage::new();
    let u32_cast = TempPackage::new();
    let source = |target_type: &str| {
        format!(
            r#"pub machine compare(value: u8)
requires (value as {target_type}) == 1
{{ }}
"#
        )
    };
    u16_cast.write("main.omg", &source("u16"));
    u32_cast.write("main.omg", &source("u32"));
    let build = r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    u16_cast.write("build.omg", build);
    u32_cast.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("exact widening cast contract should check");
        project_checked_package_review(&checked).expect("cast contract package review")
    };
    let u16_cast = project(&u16_cast);
    let u32_cast = project(&u32_cast);
    let compare = u16_cast
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "compare")
        .expect("public comparison callable");
    let [contract] = compare.contracts() else {
        panic!("one cast contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        left, ..
    }) = contract.fact()
    else {
        panic!("binary cast contract")
    };
    let PackageReviewContractExpression::Cast {
        value,
        target,
        arithmetic_domain,
        semantic_domain,
        semantic_domain_arguments,
        form,
    } = left.as_ref()
    else {
        panic!("structural cast expression")
    };
    assert_eq!(
        value.as_ref(),
        &PackageReviewContractExpression::Parameter(0)
    );
    assert!(target.canonical().contains("u16"));
    assert_eq!(*arithmetic_domain, PackageReviewArithmeticDomain::Exact);
    assert!(semantic_domain.is_none());
    assert!(semantic_domain_arguments.is_empty());
    assert_eq!(*form, PackageReviewCastForm::Value);
    assert_ne!(
        u16_cast
            .canonical_review_bytes()
            .expect("u16 cast encoding"),
        u32_cast
            .canonical_review_bytes()
            .expect("u32 cast encoding"),
        "changing the exact cast target must change package review identity",
    );
}

#[test]
fn review_casts_retain_public_semantic_domains_and_reject_private_exposure() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let public = TempPackage::new();
    public.write(
        "main.omg",
        r#"pub domain u16::Tagged;
pub machine compare(value: u8)
requires (value as u16 in Tagged) == 1
{ }
"#,
    );
    public.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &public.0.join("main.omg"),
        Some(target),
        package_inputs(&public.0),
    )
    .expect("public semantic-domain cast contract should check");
    let review = project_checked_package_review(&checked).expect("public domain cast review");
    let compare = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "compare")
        .expect("public comparison callable");
    let [contract] = compare.contracts() else {
        panic!("one public-domain cast contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        left, ..
    }) = contract.fact()
    else {
        panic!("binary public-domain cast contract")
    };
    let PackageReviewContractExpression::Cast {
        semantic_domain: Some(domain),
        ..
    } = left.as_ref()
    else {
        panic!("semantic domain cast identity")
    };
    assert_eq!(domain.path(), "u16::Tagged");
    assert_eq!(
        domain.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );

    let private = TempPackage::new();
    private.write(
        "main.omg",
        r#"domain u16::Hidden;
pub machine compare(value: u8)
requires (value as u16 in Hidden) == 1
{ }
"#,
    );
    private.write("build.omg", build);
    let diagnostics = compile_to_checked_with_packages(
        &private.0.join("main.omg"),
        Some(target),
        package_inputs(&private.0),
    )
    .expect_err("checked visibility must reject a private semantic domain in a public contract");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("public interface selects private domain")
    }));
}
