use crate::support::*;

#[test]
fn review_projects_public_data_invariants_from_exact_checked_rows() {
    let compile = |facts: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub data Ledger
where
{facts}
{{
    len: u32;
    count: u32;
}}
"#
            ),
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
        .expect("public data invariant fixture should check");
        project_checked_package_review(&checked)
            .expect("review should project the checked public data invariant")
    };

    let review = compile("    count <= len,");
    let [data] = review.public_data() else {
        panic!("one public data row")
    };
    let [
        PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
            meaning,
            operator,
            left,
            right,
        }),
    ] = data.invariants()
    else {
        panic!("one binary data invariant")
    };
    assert_eq!(meaning, &PackageReviewContractOperatorMeaning::Builtin);
    assert_eq!(*operator, PackageReviewContractBinaryOperator::LessOrEqual);
    for (expression, expected) in [
        (left.as_ref(), "Ledger::count"),
        (right.as_ref(), "Ledger::len"),
    ] {
        let PackageReviewContractExpression::Member {
            receiver,
            member,
            case_variant,
        } = expression
        else {
            panic!("data-subject field")
        };
        assert_eq!(
            receiver.as_ref(),
            &PackageReviewContractExpression::DomainSubject
        );
        assert_eq!(member.path(), expected);
        assert!(case_variant.is_none());
    }
    assert_ne!(
        review.canonical_review_bytes().unwrap(),
        compile("    count < len,")
            .canonical_review_bytes()
            .unwrap(),
        "changing a public data invariant must change canonical package identity"
    );
    assert_eq!(
        review.canonical_review_bytes().unwrap(),
        compile("    count <= len,\n    count <= len,")
            .canonical_review_bytes()
            .unwrap(),
        "duplicate invariant observations must normalize to one canonical fact"
    );
    assert_eq!(
        compile("    count <= len,\n    count <= 8,")
            .canonical_review_bytes()
            .unwrap(),
        compile("    count <= 8,\n    count <= len,")
            .canonical_review_bytes()
            .unwrap(),
        "authored invariant order must not change canonical package identity"
    );
}

#[test]
fn public_data_invariants_keep_generic_binders_distinct_from_fields() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Buffer<const N: u64>
where N <= 8,
{
    used: u64;
}
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
    .expect("generic data invariant should check");
    let review = project_checked_package_review(&checked)
        .expect("generic data invariant should retain its binder identity");
    let [data] = review.public_data() else {
        panic!("one public data row")
    };
    let [
        PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
            left, ..
        }),
    ] = data.invariants()
    else {
        panic!("one generic data invariant")
    };
    assert_eq!(
        left.as_ref(),
        &PackageReviewContractExpression::GenericBinder(0)
    );
}

#[test]
fn public_data_membership_invariants_keep_exact_field_and_domain_identity() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub domain u32::Small
requires self <= 8;

pub data Counter
where count in u32::Small,
{
    count: u32;
}
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
    .expect("data membership invariant should check");
    let review = project_checked_package_review(&checked)
        .expect("data membership invariant should retain exact identities");
    let [data] = review.public_data() else {
        panic!("one public data row")
    };
    let [PackageReviewContractFact::Membership { value, domain }] = data.invariants() else {
        panic!("one membership invariant")
    };
    let PackageReviewContractExpression::Member {
        receiver, member, ..
    } = value
    else {
        panic!("membership value projects the data field")
    };
    assert_eq!(
        receiver.as_ref(),
        &PackageReviewContractExpression::DomainSubject
    );
    assert_eq!(member.path(), "Counter::count");
    assert_eq!(domain.path(), "u32::Small");
}

#[test]
fn public_data_invariant_review_rejects_checked_ownership_spoofs() {
    let compile = || {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            r#"pub data Ledger
where count <= len,
{
    len: u32;
    count: u32;
}
"#,
        );
        package.write(
            "build.omg",
            "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x86_64"),
            package_inputs(&package.0),
        )
        .expect("public data ownership fixture should check")
    };
    let assert_rejects = |checked: &_, expected: &str| {
        let diagnostics = project_checked_package_review(checked)
            .expect_err("spoofed checked data ownership must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing `{expected}` in {diagnostics:#?}"
        );
    };

    let mut missing_owner = compile();
    let owner = missing_owner
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("data ownership record");
    assert!(
        missing_owner
            .facts
            .semantic
            .data_definition_facts
            .free(owner)
    );
    assert_rejects(&missing_owner, "data invariant evidence");

    let mut duplicate_owner = compile();
    let owner = duplicate_owner
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.clone())
        .expect("data ownership record");
    duplicate_owner
        .facts
        .semantic
        .data_definition_facts
        .append(owner);
    assert_rejects(&duplicate_owner, "data invariant evidence");

    let mut unrelated_extra_owner = compile();
    let mut owner = unrelated_extra_owner
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.clone())
        .expect("data ownership record");
    owner.semantic_fact = Default::default();
    unrelated_extra_owner
        .facts
        .semantic
        .data_definition_facts
        .append(owner);
    assert_rejects(&unrelated_extra_owner, "data invariant evidence");

    let mut wrong_origin = compile();
    let semantic_fact = wrong_origin
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.semantic_fact)
        .expect("data semantic fact");
    wrong_origin
        .facts
        .semantic
        .facts
        .get_mut(semantic_fact)
        .origin = psi_facts::FactOrigin::Unknown;
    assert_rejects(&wrong_origin, "data invariant evidence");

    let mut missing_dependency = compile();
    let owner = missing_dependency
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("data ownership record");
    missing_dependency
        .facts
        .semantic
        .data_definition_facts
        .get_mut(owner)
        .dependencies
        .clear();
    assert_rejects(&missing_dependency, "data invariant evidence");

    let mut extra_dependency = compile();
    let owner = extra_dependency
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("data ownership record");
    let dependency = extra_dependency
        .facts
        .semantic
        .data_definition_facts
        .get(owner)
        .dependencies[0];
    extra_dependency
        .facts
        .semantic
        .data_definition_facts
        .get_mut(owner)
        .dependencies
        .push(dependency);
    assert_rejects(&extra_dependency, "data invariant evidence");

    let mut orphan_semantic_fact = compile();
    let semantic_fact = orphan_semantic_fact
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.semantic_fact)
        .expect("data semantic fact");
    let fact = *orphan_semantic_fact.facts.semantic.facts.get(semantic_fact);
    orphan_semantic_fact.facts.semantic.facts.append(fact);
    assert_rejects(&orphan_semantic_fact, "data invariant evidence");

    let mut orphan_ref = compile();
    let semantic_fact = orphan_ref
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.semantic_fact)
        .expect("data semantic fact");
    orphan_ref.facts.semantic.refs.append(psi_facts::FactRef {
        fact: semantic_fact,
    });
    assert_rejects(&orphan_ref, "data invariant evidence");

    let mut dangling_ref = compile();
    dangling_ref.facts.semantic.refs.append(psi_facts::FactRef {
        fact: psi_arena::Handle::from_parts(u32::MAX, 1),
    });
    assert_rejects(&dangling_ref, "data invariant evidence");

    let mut malformed_extra_context = compile();
    malformed_extra_context
        .facts
        .semantic
        .contexts
        .append(psi_facts::FactContext {
            point: psi_facts::ProgramPoint::Global,
            facts: psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_parts(u32::MAX, 1), 1),
        });
    assert_rejects(&malformed_extra_context, "data invariant evidence");

    let mut missing_context = compile();
    let context = missing_context
        .facts
        .semantic
        .contexts
        .iter()
        .find_map(|(handle, context)| {
            matches!(context.point, psi_facts::ProgramPoint::Definition { .. }).then_some(handle)
        })
        .expect("data fact context");
    assert!(missing_context.facts.semantic.contexts.free(context));
    assert_rejects(&missing_context, "data invariant evidence");

    let mut missing_symbol_set = compile();
    let symbol_set = missing_symbol_set
        .facts
        .semantic
        .symbol_sets
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("data symbol fact set");
    assert!(
        missing_symbol_set
            .facts
            .semantic
            .symbol_sets
            .free(symbol_set)
    );
    assert_rejects(&missing_symbol_set, "data invariant evidence");

    let mut malformed_empty_path = {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            r#"pub data Buffer<const N: u64>
where N <= 8,
{
    used: u64;
}
"#,
        );
        package.write(
            "build.omg",
            "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x86_64"),
            package_inputs(&package.0),
        )
        .expect("generic data ownership fixture should check")
    };
    let binder_place = malformed_empty_path
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.dependencies[0].place)
        .expect("generic binder dependency");
    assert!(
        malformed_empty_path
            .facts
            .semantic
            .places
            .get(binder_place)
            .segments
            .is_empty()
    );
    malformed_empty_path
        .facts
        .semantic
        .places
        .get_mut(binder_place)
        .segments =
        psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_parts(u32::MAX, 1), 1);
    assert_rejects(&malformed_empty_path, "data invariant evidence");
}
