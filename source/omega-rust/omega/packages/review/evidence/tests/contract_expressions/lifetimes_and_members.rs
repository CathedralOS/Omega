use crate::support::*;

#[test]
fn review_alpha_normalizes_lifetime_bearing_nested_type_arguments() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    let changed = TempPackage::new();
    let source = |view_lifetime: &str, left: &str, right: &str, selected: &str| {
        format!(
            r#"pub data View<'{view_lifetime}, Value> {{ value: &'{view_lifetime} Value; }}
pub machine tag<Value>() -> u64 {{ 0 }}
pub machine generic_tag<'{left}, '{right}>(
    first: &'{left} u64,
    second: &'{right} u64
) -> u64
requires tag<View<'{selected}, u64>>() == tag<View<'{selected}, u64>>()
{{
    0
}}
"#,
        )
    };
    original.write("main.omg", &source("slot", "left", "right", "left"));
    renamed.write(
        "main.omg",
        &source("renamed_slot", "primary", "secondary", "primary"),
    );
    changed.write("main.omg", &source("slot", "left", "right", "right"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    for package in [&original, &renamed, &changed] {
        package.write("build.omg", build);
    }
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("lifetime-bearing nested type contract call should check");
        project_checked_package_review(&checked)
            .expect("nested lifetime arguments have canonical binder ordinals")
    };
    let original = project(&original);
    let renamed = project(&renamed);
    let changed = project(&changed);
    let generic_tag = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "generic_tag")
        .expect("generic tag callable");
    let [contract] = generic_tag.contracts() else {
        panic!("one generic-tag contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("generic-tag equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("generic tag call")
    };
    let [
        PackageReviewContractStaticArgument::GenericType {
            lifetime_arguments, ..
        },
    ] = static_arguments.as_slice()
    else {
        panic!("one lifetime-bearing generic data argument")
    };
    assert_eq!(lifetime_arguments, &[0]);
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        renamed.canonical_review_bytes().unwrap(),
        "renaming caller and data lifetime binders must preserve package-review identity",
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed.canonical_review_bytes().unwrap(),
        "selecting a different caller lifetime must change package-review identity",
    );
}

#[test]
fn review_projects_contract_member_paths_with_exact_receivers_and_fields() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let changed = TempPackage::new();
    let source = |left_receiver: &str, right_receiver: &str| {
        format!(
            r#"pub data Pair [copy] {{
    left: i32;
    right: i32;
}}
pub machine compare(first: Pair, second: Pair)
requires {left_receiver}.left == {right_receiver}.right
{{ }}
"#
        )
    };
    original.write("main.omg", &source("first", "second"));
    changed.write("main.omg", &source("second", "first"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    original.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("member-path contract fixture should check");
        project_checked_package_review(&checked).expect("member-path package review")
    };
    let original = project(&original);
    let changed = project(&changed);
    let compare = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "compare")
        .expect("public comparison callable");
    let [contract] = compare.contracts() else {
        panic!("one member-path contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        left,
        right,
        ..
    }) = contract.fact()
    else {
        panic!("binary member-path contract")
    };
    let PackageReviewContractExpression::Member {
        receiver: left_receiver,
        member: left_member,
        case_variant: left_variant,
    } = left.as_ref()
    else {
        panic!("left member path")
    };
    let PackageReviewContractExpression::Member {
        receiver: right_receiver,
        member: right_member,
        case_variant: right_variant,
    } = right.as_ref()
    else {
        panic!("right member path")
    };
    assert_eq!(
        left_receiver.as_ref(),
        &PackageReviewContractExpression::Parameter(0)
    );
    assert_eq!(
        right_receiver.as_ref(),
        &PackageReviewContractExpression::Parameter(1)
    );
    assert_eq!(left_member.path(), "Pair::left");
    assert_eq!(right_member.path(), "Pair::right");
    assert!(left_variant.is_none());
    assert!(right_variant.is_none());
    assert_eq!(
        left_member.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_ne!(
        original
            .canonical_review_bytes()
            .expect("original member-path encoding"),
        changed
            .canonical_review_bytes()
            .expect("changed member-path encoding"),
        "changing only the receiver coordinates must change package review identity",
    );
}

#[test]
fn review_rejects_nominal_member_selection_custody_tamper() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Pair [copy] {
    left: i32;
    right: i32;
}
pub proposition balanced(pair: Pair) = pair.left == pair.right;
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public nominal-member proposition should check");
    project_checked_package_review(&checked)
        .expect("untampered nominal-member selection custody should project");

    let members = checked
        .expression_table
        .iter_expressions()
        .filter_map(|(expression, node)| {
            let psi_typed_trees::expression::ExpressionNode::Member(member) = node else {
                return None;
            };
            matches!(member.member.as_str(), "left" | "right").then_some((
                member.member.as_str().to_owned(),
                expression,
                checked
                    .expression_table
                    .authored_selection_occurrences(expression)
                    .collect::<Vec<_>>(),
            ))
        })
        .collect::<Vec<_>>();
    let left = members
        .iter()
        .find(|(name, _, _)| name == "left")
        .expect("left member expression");
    let right = members
        .iter()
        .find(|(name, _, _)| name == "right")
        .expect("right member expression");
    let [left_occurrence] = left.2.as_slice() else {
        panic!("left member must retain one exact selection")
    };
    let [right_occurrence] = right.2.as_slice() else {
        panic!("right member must retain one exact selection")
    };
    assert_ne!(left_occurrence, right_occurrence);

    let mut tampered = checked.clone();
    tampered
        .typed
        .expression_table
        .attach_authored_selection_occurrences(left.1, [*right_occurrence]);
    let diagnostics = project_checked_package_review(&tampered)
        .expect_err("duplicate nominal-member selection custody must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("nominal member has 2 exact checked member-selection rows")
    }));
}

#[test]
fn review_projects_computed_nominal_member_receivers() {
    let project = |selected: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub data Pair [copy] {{
    left: i32;
    right: i32;
}}
pub proposition selects_computed_member(value: i32) =
    (Pair {{ left: value, right: value }}).{selected} == value;
"#,
            ),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("computed nominal-member proposition should check");
        project_checked_package_review(&checked)
            .expect("computed nominal-member receiver should project")
    };

    let left_review = project("left");
    let right_review = project("right");
    let proposition = left_review
        .public_propositions()
        .iter()
        .find(|proposition| proposition.identity().path() == "selects_computed_member")
        .expect("computed-member proposition");
    let PackageReviewPublicPropositionBody::Transparent(PackageReviewContractFact::Expression(
        PackageReviewContractExpression::Binary { left, .. },
    )) = proposition.body()
    else {
        panic!("computed-member proposition definition")
    };
    let PackageReviewContractExpression::Member {
        receiver, member, ..
    } = left.as_ref()
    else {
        panic!("computed member expression")
    };
    assert!(matches!(
        receiver.as_ref(),
        PackageReviewContractExpression::Constructor { .. }
    ));
    assert_eq!(member.path(), "Pair::left");
    assert_ne!(
        left_review.canonical_review_bytes().unwrap(),
        right_review.canonical_review_bytes().unwrap(),
        "changing the exact member selected from an otherwise identical computed receiver must change review identity",
    );

    let case_package = TempPackage::new();
    case_package.write(
        "main.omg",
        r#"pub data Outcome [copy] {
    case Left(value: i32);
    case Right(value: i32);
}
pub proposition selects_case_member(value: i32) =
    (Outcome::Right { value: value }).value == value;
"#,
    );
    case_package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &case_package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&case_package.0),
    )
    .expect("computed case-member proposition should check");
    let case_review = project_checked_package_review(&checked)
        .expect("computed case-member receiver should project");
    let proposition = case_review
        .public_propositions()
        .iter()
        .find(|proposition| proposition.identity().path() == "selects_case_member")
        .expect("computed case-member proposition");
    let PackageReviewPublicPropositionBody::Transparent(PackageReviewContractFact::Expression(
        PackageReviewContractExpression::Binary { left, .. },
    )) = proposition.body()
    else {
        panic!("computed case-member proposition definition")
    };
    let PackageReviewContractExpression::Member {
        member,
        case_variant: Some(case_variant),
        ..
    } = left.as_ref()
    else {
        panic!("computed case member expression")
    };
    assert_eq!(member.path(), "Outcome::Right::value");
    assert_eq!(case_variant.path(), "Outcome::Right");
}

#[test]
fn review_projects_computed_nominal_member_receivers_in_checked_contracts() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let compile = |selected: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub data Pair [copy] {{
    left: i32;
    right: i32;
}}
pub machine accepts_computed_member(value: i32)
requires (Pair {{ left: value, right: value }}).{selected} == value
{{}}
"#,
            ),
        );
        package.write("build.omg", build);
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("computed nominal-member machine contract should check")
    };

    let left_checked = compile("left");
    let left_review = project_checked_package_review(&left_checked)
        .expect("computed nominal-member machine contract should project");
    let right_review = project_checked_package_review(&compile("right"))
        .expect("changed computed nominal-member machine contract should project");
    let callable = left_review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "accepts_computed_member")
        .expect("computed-member callable");
    let [contract] = callable.contracts() else {
        panic!("one computed-member contract")
    };
    assert_eq!(contract.kind(), PackageReviewContractKind::Requires);
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        left, ..
    }) = contract.fact()
    else {
        panic!("computed-member equality contract")
    };
    let PackageReviewContractExpression::Member {
        receiver, member, ..
    } = left.as_ref()
    else {
        panic!("computed member contract expression")
    };
    assert!(matches!(
        receiver.as_ref(),
        PackageReviewContractExpression::Constructor { .. }
    ));
    assert_eq!(member.path(), "Pair::left");
    assert_ne!(
        left_review.canonical_review_bytes().unwrap(),
        right_review.canonical_review_bytes().unwrap(),
        "changing the exact member selected by a checked contract must change review identity",
    );

    let case_package = TempPackage::new();
    case_package.write(
        "main.omg",
        r#"pub data Outcome [copy] {
    case Left(value: i32);
    case Right(value: i32);
}
pub machine accepts_case_member(value: i32)
requires (Outcome::Right { value: value }).value == value
{}
"#,
    );
    case_package.write("build.omg", build);
    let case_checked = compile_to_checked_with_packages(
        &case_package.0.join("main.omg"),
        Some(target),
        package_inputs(&case_package.0),
    )
    .expect("computed case-member machine contract should check");
    let case_review = project_checked_package_review(&case_checked)
        .expect("computed case-member machine contract should project");
    let callable = case_review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "accepts_case_member")
        .expect("computed case-member callable");
    let [contract] = callable.contracts() else {
        panic!("one computed case-member contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        left, ..
    }) = contract.fact()
    else {
        panic!("computed case-member equality contract")
    };
    let PackageReviewContractExpression::Member {
        member,
        case_variant: Some(case_variant),
        ..
    } = left.as_ref()
    else {
        panic!("computed case-member contract expression")
    };
    assert_eq!(member.path(), "Outcome::Right::value");
    assert_eq!(case_variant.path(), "Outcome::Right");

    let custody_package = TempPackage::new();
    custody_package.write(
        "main.omg",
        r#"pub data Pair [copy] {
    left: i32;
    right: i32;
}
pub machine compares_computed_members(value: i32)
requires (Pair { left: value, right: value }).left ==
         (Pair { left: value, right: value }).right
{}
"#,
    );
    custody_package.write("build.omg", build);
    let mut custody_checked = compile_to_checked_with_packages(
        &custody_package.0.join("main.omg"),
        Some(target),
        package_inputs(&custody_package.0),
    )
    .expect("computed-member custody fixture should check");
    let members = custody_checked
        .expression_table
        .iter_expressions()
        .filter_map(|(expression, node)| {
            let psi_typed_trees::expression::ExpressionNode::Member(member) = node else {
                return None;
            };
            matches!(member.member.as_str(), "left" | "right").then_some((
                member.member.as_str().to_owned(),
                expression,
                custody_checked
                    .expression_table
                    .authored_selection_occurrences(expression)
                    .collect::<Vec<_>>(),
            ))
        })
        .collect::<Vec<_>>();
    let left = members
        .iter()
        .find(|(name, _, _)| name == "left")
        .expect("computed left member");
    let right = members
        .iter()
        .find(|(name, _, _)| name == "right")
        .expect("computed right member");
    let [right_occurrence] = right.2.as_slice() else {
        panic!("computed right member must retain one exact selection")
    };
    custody_checked
        .typed
        .expression_table
        .attach_authored_selection_occurrences(left.1, [*right_occurrence]);
    let diagnostics = project_checked_package_review(&custody_checked)
        .expect_err("computed-member selection custody drift must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("nominal member has 2 exact checked member-selection rows")
    }));
}
