mod support;

use support::*;

#[test]
fn review_projects_exact_concrete_type_arguments_in_contract_calls() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    let changed = TempPackage::new();
    let source = |selected_type: &str| {
        format!(
            r#"pub machine tag<Value>() -> u64 {{ 0 }}
boundary machine trusted_zero() -> u64
ensures result == tag<{selected_type}>();
"#,
        )
    };
    package.write("main.omg", &source("u64"));
    changed.write("main.omg", &source("i64"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    package.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("effect-free static type contract call should check");
        project_checked_package_review(&checked)
            .expect("a direct concrete type argument has a canonical contract row")
    };
    let review = project(&package);
    let changed = project(&changed);
    let trusted_zero = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "trusted_zero")
        .expect("trusted boundary callable");
    let [contract] = trusted_zero.contracts() else {
        panic!("one trusted-zero contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("trusted-zero equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("generic tag call")
    };
    let [PackageReviewContractStaticArgument::Type(identity)] = static_arguments.as_slice() else {
        panic!("one exact concrete type argument")
    };
    assert!(identity.canonical().contains("u64"));
    assert_ne!(
        review
            .canonical_review_bytes()
            .expect("u64 static-type contract encoding"),
        changed
            .canonical_review_bytes()
            .expect("i64 static-type contract encoding"),
        "changing an exact concrete type selection must change package-review identity",
    );
}

#[test]
fn review_projects_canonical_integer_const_arguments_in_contract_calls() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    let changed = TempPackage::new();
    let source = |selected_value: &str| {
        format!(
            r#"pub machine constant<const Value: u64>() -> u64 {{ 7 }}
boundary machine trusted_constant() -> u64
ensures result == constant<{selected_value}>();
"#,
        )
    };
    package.write("main.omg", &source("0x07"));
    changed.write("main.omg", &source("0x08"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    package.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("effect-free const-generic contract call should check");
        project_checked_package_review(&checked)
            .expect("a direct integer const argument has a canonical contract row")
    };
    let review = project(&package);
    let changed = project(&changed);
    let trusted_constant = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "trusted_constant")
        .expect("trusted const boundary callable");
    let [contract] = trusted_constant.contracts() else {
        panic!("one trusted-constant contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("trusted-constant equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("const-generic call")
    };
    assert_eq!(
        static_arguments,
        &[PackageReviewContractStaticArgument::ConstInteger(
            "0x7".to_owned()
        )]
    );
    assert_ne!(
        review
            .canonical_review_bytes()
            .expect("0x7 static-const contract encoding"),
        changed
            .canonical_review_bytes()
            .expect("0x8 static-const contract encoding"),
        "changing an exact const selection must change package-review identity",
    );
}

#[test]
fn review_alpha_normalizes_forwarded_type_and_const_binders() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    let changed_type = TempPackage::new();
    let changed_const = TempPackage::new();
    let source = |first: &str,
                  second: &str,
                  left: &str,
                  right: &str,
                  selected_type: &str,
                  selected_const: &str| {
        format!(
            r#"pub machine tag<Value>() -> u64 {{ 0 }}
pub machine constant<const Value: u64>() -> u64 {{ 0 }}
pub machine generic_type<{first}, {second}>() -> u64
requires tag<{selected_type}>() == tag<{selected_type}>()
{{
    0
}}
pub machine generic_const<const {left}: u64, const {right}: u64>() -> u64
requires constant<{selected_const}>() == constant<{selected_const}>()
{{
    0
}}
"#,
        )
    };
    original.write(
        "main.omg",
        &source("First", "Second", "Left", "Right", "First", "Left"),
    );
    renamed.write(
        "main.omg",
        &source(
            "Primary",
            "Secondary",
            "Minimum",
            "Maximum",
            "Primary",
            "Minimum",
        ),
    );
    changed_type.write(
        "main.omg",
        &source("First", "Second", "Left", "Right", "Second", "Left"),
    );
    changed_const.write(
        "main.omg",
        &source("First", "Second", "Left", "Right", "First", "Right"),
    );
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    for package in [&original, &renamed, &changed_type, &changed_const] {
        package.write("build.omg", build);
    }
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("forwarded type and const contract arguments should check");
        project_checked_package_review(&checked)
            .expect("forwarded type and const binders have canonical review rows")
    };
    let original = project(&original);
    let renamed = project(&renamed);
    let changed_type = project(&changed_type);
    let changed_const = project(&changed_const);
    let static_arguments = |name: &str| {
        let callable = original
            .callables()
            .iter()
            .find(|callable| callable.identity().path() == name)
            .expect("generic callable");
        let [contract] = callable.contracts() else {
            panic!("one generic callable contract")
        };
        let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
            right,
            ..
        }) = contract.fact()
        else {
            panic!("generic callable equality contract")
        };
        let PackageReviewContractExpression::Call {
            static_arguments, ..
        } = right.as_ref()
        else {
            panic!("generic callable contract call")
        };
        static_arguments.clone()
    };
    assert_eq!(
        static_arguments("generic_type"),
        [PackageReviewContractStaticArgument::GenericTypeBinder(0)]
    );
    assert_eq!(
        static_arguments("generic_const"),
        [PackageReviewContractStaticArgument::GenericConstBinder(0)]
    );
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        renamed.canonical_review_bytes().unwrap(),
        "renaming forwarded type and const binders must preserve review identity",
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_type.canonical_review_bytes().unwrap(),
        "selecting a different forwarded type binder must change review identity",
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_const.canonical_review_bytes().unwrap(),
        "selecting a different forwarded const binder must change review identity",
    );
}

#[test]
fn review_projects_recursive_generic_data_arguments_in_contract_calls() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    let changed = TempPackage::new();
    let source = |nested_type: &str| {
        format!(
            r#"pub data Wrapper<Value> {{ value: Value; }}
pub machine tag<Value>() -> u64 {{ 0 }}
boundary machine trusted_tag() -> u64
ensures result == tag<Wrapper<{nested_type}>>();
"#,
        )
    };
    package.write("main.omg", &source("u64"));
    changed.write("main.omg", &source("i64"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    package.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("nested static type contract call should check");
        project_checked_package_review(&checked)
            .expect("a recursive generic data argument has a canonical contract row")
    };
    let review = project(&package);
    let changed = project(&changed);
    let trusted_tag = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "trusted_tag")
        .expect("trusted tag boundary callable");
    let [contract] = trusted_tag.contracts() else {
        panic!("one trusted-tag contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("trusted-tag equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("generic tag call")
    };
    let [
        PackageReviewContractStaticArgument::GenericType {
            base,
            lifetime_arguments,
            arguments,
        },
    ] = static_arguments.as_slice()
    else {
        panic!("one generic data static argument")
    };
    assert!(base.canonical().contains("Wrapper"));
    assert!(lifetime_arguments.is_empty());
    let [PackageReviewContractStaticArgument::Type(nested)] = arguments.as_slice() else {
        panic!("one nested concrete type argument")
    };
    assert!(nested.canonical().contains("u64"));
    assert_ne!(
        review
            .canonical_review_bytes()
            .expect("Wrapper<u64> contract encoding"),
        changed
            .canonical_review_bytes()
            .expect("Wrapper<i64> contract encoding"),
        "changing a nested concrete type must change package-review identity",
    );
}

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
fn review_projects_collection_length_as_an_exact_compiler_intrinsic() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        "pub proposition non_empty(items: &[u8]) = items.len > 0;\n",
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
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
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
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
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
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
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
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
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

#[test]
fn public_callable_signatures_are_exact_and_lifetime_alpha_normalized() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    let changed = TempPackage::new();
    original.write(
        "main.omg",
        r#"pub machine borrow<'source, 'temporary>(
    source: &'source [u8],
    temporary: &'temporary [u8]
) -> &'source [u8] { source }
pub machine identity<Element [copy]>(value: Element) -> Element { value }
"#,
    );
    renamed.write(
        "main.omg",
        r#"pub machine borrow<'origin, 'scratch>(
    source: &'origin [u8],
    temporary: &'scratch [u8]
) -> &'origin [u8] { source }
pub machine identity<Value [copy]>(value: Value) -> Value { value }
"#,
    );
    changed.write(
        "main.omg",
        r#"pub machine borrow<'source, 'temporary>(
    source: &'source [u8],
    temporary: &'temporary [u8]
) -> &'temporary [u8] { temporary }
pub machine identity<Element [copy]>(value: Element) -> Element { value }
"#,
    );
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    original.write("build.omg", build);
    renamed.write("build.omg", build);
    changed.write("build.omg", build);

    let review = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("public callable signature fixture should check");
        project_checked_package_review(&checked).expect("callable signature review should close")
    };
    let original = review(&original);
    let renamed = review(&renamed);
    let changed = review(&changed);
    let borrow = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("borrow"))
        .expect("borrow callable row");
    assert_eq!(borrow.lifetime_parameter_count(), 2);
    assert_eq!(borrow.parameters().len(), 2);
    assert!(borrow.type_parameters().is_empty());
    assert!(!borrow.return_type().canonical().is_empty());
    assert!(
        borrow.parameters()[0]
            .type_identity()
            .canonical()
            .contains("compiler-type"),
        "source-free builtin u8 must use a closed compiler atom: {}",
        borrow.parameters()[0].type_identity().canonical(),
    );
    assert!(
        !borrow.parameters()[0]
            .type_identity()
            .canonical()
            .contains("unresolved-owner"),
        "compiler builtins must not remain unresolved in package review",
    );
    let identity = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("identity"))
        .expect("generic identity callable row");
    assert_eq!(identity.type_parameters().len(), 1);
    assert_eq!(identity.parameters().len(), 1);

    assert_eq!(
        original
            .canonical_review_bytes()
            .expect("original encoding"),
        renamed.canonical_review_bytes().expect("renamed encoding"),
        "renaming lifetime and type binders must not alter canonical review evidence",
    );
    assert_ne!(
        original
            .canonical_review_bytes()
            .expect("original encoding"),
        changed.canonical_review_bytes().expect("changed encoding"),
        "changing the result's borrow relationship must alter canonical review evidence",
    );
}

#[test]
fn public_signatures_encode_closed_compiler_domains_and_exact_layout_schema() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Save {
    #1 value: u32;
}

pub machine inspect(
    number: f64 in Finite,
    token: u64 in Carry::AnyCpu,
    bytes: &[u8] in OmegaLayout<Save>
) { }
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("closed compiler-domain fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("closed compiler domains should project without textual fallback");
    let inspect = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("inspect"))
        .expect("inspect callable review row");
    let [number, token, bytes] = inspect.parameters() else {
        panic!("three inspect parameters")
    };
    assert!(
        number
            .type_identity()
            .canonical()
            .contains("compiler-domain")
    );
    assert!(number.type_identity().canonical().contains("finite"));
    assert!(
        !number
            .type_identity()
            .canonical()
            .contains("unresolved-owner")
    );
    assert!(
        token
            .type_identity()
            .canonical()
            .contains("compiler-domain")
    );
    assert!(token.type_identity().canonical().contains("any-cpu"));
    assert!(
        !token
            .type_identity()
            .canonical()
            .contains("unresolved-owner")
    );
    assert!(bytes.type_identity().canonical().contains("omega-layout"));
    assert!(bytes.type_identity().canonical().contains("derived"));
    assert!(bytes.type_identity().canonical().contains("Save"));
    assert!(bytes.type_identity().canonical().contains("package-owner"));
    assert!(
        !bytes
            .type_identity()
            .canonical()
            .contains("unresolved-owner")
    );
}

#[test]
fn public_signatures_encode_structured_const_values_without_transport_or_display_text() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data UnitIndex { scale: u64; exponent: i32; }
data UnitIndices {}
const UnitIndices::Meters: UnitIndex = UnitIndex { scale: 1, exponent: 0 };

pub domain<Carrier, const Index: UnitIndex> Carrier::Quantity<Index>;
pub domain<Carrier, const Count: u64> Carrier::Counted<Count>;

pub data Reading {
    value: i64 in Quantity<UnitIndices::Meters>;
    count: i64 in Counted<7>;
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("structured const package fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("structured const value should project through closed identity");
    let reading = review
        .public_data()
        .iter()
        .find(|data| data.identity().path().contains("Reading"))
        .expect("Reading review row");
    let field = |name| {
        reading
            .members()
            .iter()
            .find_map(|member| match member {
                PackageReviewDataMember::Field(field) if field.name() == name => Some(field),
                PackageReviewDataMember::Field(_) | PackageReviewDataMember::Variant { .. } => None,
            })
            .unwrap_or_else(|| panic!("Reading field `{name}`"))
    };
    let identity = field("value").type_identity().canonical();

    assert!(identity.contains("canonical-const"), "{identity}");
    assert!(identity.contains("encoding"), "{identity}");
    assert!(!identity.contains("#omega-const"), "{identity}");
    assert!(!identity.contains("UnitIndex {"), "{identity}");
    assert!(!identity.contains("unresolved-owner"), "{identity}");
    let integer = field("count").type_identity().canonical();
    assert!(integer.contains("integer-const"), "{integer}");
    assert!(integer.contains('7'), "{integer}");
    assert!(!integer.contains("unresolved-owner"), "{integer}");
}
