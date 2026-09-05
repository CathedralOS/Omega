use crate::support::*;

#[test]
fn review_projects_width_landed_float_literals_by_exact_bits() {
    let project = |parameter_type: &str, literal: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!("pub proposition reviewed(value: {parameter_type}) = value == {literal};\n"),
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
        .expect("width-landed float-literal contract should check");
        project_checked_package_review(&checked)
            .expect("width-landed float literal should have exact review identity")
    };

    let f32_review = project("f32", "1.25");
    let f64_review = project("f64", "1.25");
    let equivalent_f64_review = project("f64", "1.2500");
    let changed_f64_review = project("f64", "1.5");

    let [proposition] = f32_review.public_propositions() else {
        panic!("one public proposition")
    };
    let PackageReviewPublicPropositionBody::Transparent(PackageReviewContractFact::Expression(
        PackageReviewContractExpression::Binary { right, .. },
    )) = proposition.body()
    else {
        panic!("transparent float comparison")
    };
    assert_eq!(
        right.as_ref(),
        &PackageReviewContractExpression::Float(PackageReviewFloatLiteral::F32(1.25f32.to_bits(),)),
    );

    assert_ne!(
        f32_review.canonical_review_bytes().unwrap(),
        f64_review.canonical_review_bytes().unwrap(),
        "float format is part of package-review contract identity",
    );
    assert_eq!(
        f64_review.canonical_review_bytes().unwrap(),
        equivalent_f64_review.canonical_review_bytes().unwrap(),
        "equivalent decimal spellings must normalize to the same landed float identity",
    );
    assert_ne!(
        f64_review.canonical_review_bytes().unwrap(),
        changed_f64_review.canonical_review_bytes().unwrap(),
        "landed float bits are part of package-review contract identity",
    );
}

#[test]
fn float_contract_review_rejects_missing_checked_width_landing() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        "pub proposition reviewed(value: f32) = value == 1.25;\n",
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
    .expect("float landing-tamper fixture should check");
    let float_expression = checked
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| {
            matches!(node, typed_trees::expression::ExpressionNode::Float(_)).then_some(expression)
        })
        .expect("landed float expression");
    *checked
        .typed
        .expression_table
        .expression_mut(float_expression) = typed_trees::expression::ExpressionNode::Float(
        typed_trees::expression::FloatLiteral::parse("1.25").expect("unlanded exact float literal"),
    );

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("review must not infer a missing checked float width");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("without an exact checked width landing")
    }));
}

#[test]
fn review_projects_result_typed_float_contract_literals() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary machine measured() -> f32
ensures result == 1.25
;
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
    .expect("result-typed float contract should check");
    let review = project_checked_package_review(&checked)
        .expect("result-typed float contract should have exact review identity");
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Boundary)
        .expect("public measured callable");
    let ensures = callable
        .contracts()
        .iter()
        .find(|contract| contract.kind() == PackageReviewContractKind::Ensures)
        .expect("measured ensures contract");
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = ensures.fact()
    else {
        panic!("one result float comparison")
    };
    assert_eq!(
        right.as_ref(),
        &PackageReviewContractExpression::Float(PackageReviewFloatLiteral::F32(1.25f32.to_bits())),
    );
}

#[test]
fn review_lands_result_float_contracts_for_trait_and_operator_declarations() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Scalar {}

pub boundary operator Scalar::measure() -> f32
ensures result == 1.25;

pub trait Measures {
    machine measured() -> f64
    ensures result == 2.5;
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
    .expect("trait and operator result-float contracts should check");
    project_checked_package_review(&checked)
        .expect("trait and operator result-float contracts should retain exact landings");
}

#[test]
fn review_projects_exact_compiler_byte_sequence_predicate_identity() {
    let project = |predicate: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!("pub domain [u8]::ReviewedBytes\nrequires\n    {predicate}(self);\n"),
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
        .expect("compiler-owned byte predicate should check");
        project_checked_package_review(&checked)
            .expect("compiler-owned byte predicate should have exact review identity")
    };

    let mut encodings = Vec::new();
    for (name, expected) in [
        ("valid_utf8", PackageReviewByteSequencePredicate::ValidUtf8),
        ("no_nul", PackageReviewByteSequencePredicate::NoNul),
        ("ascii_only", PackageReviewByteSequencePredicate::AsciiOnly),
        ("non_empty", PackageReviewByteSequencePredicate::NonEmpty),
    ] {
        let review = project(name);
        let [domain] = review.public_domains() else {
            panic!("one byte-domain row")
        };
        let [
            PackageReviewContractFact::Expression(PackageReviewContractExpression::Call {
                target,
                static_arguments,
                arguments,
                ..
            }),
        ] = domain.predicate_facts()
        else {
            panic!("one byte-predicate call")
        };
        assert_eq!(target.byte_sequence_predicate(), Some(expected));
        assert!(static_arguments.is_empty());
        assert_eq!(arguments, &[PackageReviewContractExpression::DomainSubject]);
        encodings.push(review.canonical_review_bytes().unwrap());
    }
    encodings.sort();
    encodings.dedup();
    assert_eq!(
        encodings.len(),
        4,
        "each exact compiler predicate must have distinct package-review identity"
    );
}

#[test]
fn review_projects_exact_compiler_builtin_function_identity() {
    let project = |expression: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!("pub proposition reviewed(left: f64, right: f64) = {expression};\n"),
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
        .expect("compiler builtin-function contract should check");
        project_checked_package_review(&checked)
            .expect("compiler builtin function should have exact review identity")
    };

    let mut encodings = Vec::new();
    for (expression, expected) in [
        ("min(left, right) == left", symbols::BuiltinFunction::Min),
        ("max(left, right) == right", symbols::BuiltinFunction::Max),
        ("sqrt(left) == right", symbols::BuiltinFunction::Sqrt),
    ] {
        let review = project(expression);
        let [proposition] = review.public_propositions() else {
            panic!("one public proposition")
        };
        let PackageReviewPublicPropositionBody::Transparent(PackageReviewContractFact::Expression(
            PackageReviewContractExpression::Binary { left, .. },
        )) = proposition.body()
        else {
            panic!("one transparent binary proposition")
        };
        let PackageReviewContractExpression::Call {
            target,
            static_arguments,
            ..
        } = left.as_ref()
        else {
            panic!("builtin function call on the left side")
        };
        assert_eq!(target.builtin_function(), Some(expected));
        assert!(target.nominal().is_none());
        assert!(static_arguments.is_empty());
        let row = review
            .canonical_rows()
            .expect("builtin-function canonical rows")
            .into_iter()
            .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicProposition)
            .expect("builtin-function public proposition row");
        let recovered = decode_package_review_canonical_row(
            &encode_package_review_canonical_row(&row)
                .expect("builtin-function recovery envelope should encode"),
        )
        .expect("builtin-function recovery envelope should decode");
        assert_eq!(recovered.canonical_bytes(), row.canonical_bytes());
        encodings.push(review.canonical_review_bytes().unwrap());
    }
    encodings.sort();
    encodings.dedup();
    assert_eq!(
        encodings.len(),
        3,
        "each exact compiler builtin function must have distinct package-review identity"
    );
}

#[test]
fn builtin_function_review_rejects_checked_target_symbol_tamper() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        "pub proposition reviewed(left: u64, right: u64) = min(left, right) == left;\n",
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
    .expect("builtin target-tamper fixture should check");
    let call_expression = checked
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| {
            matches!(node, typed_trees::expression::ExpressionNode::Call(_)).then_some(expression)
        })
        .expect("builtin call expression");
    let typed_trees::expression::ExpressionNode::Call(call) = checked
        .typed
        .expression_table
        .expression_mut(call_expression)
    else {
        panic!("call expression")
    };
    call.target_symbol = symbols::SymbolHandle::invalid();

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("builtin target-symbol tamper must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("contract call target disagrees with its exact checked call-selection row")
    }));
}

#[test]
fn review_projects_exact_raw_byte_literals_in_public_contracts() {
    let project = |literal: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!("pub domain [u8]::LiteralCheck\nrequires\n    no_nul({literal});\n"),
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
        .expect("raw-byte contract literal should check");
        let review = project_checked_package_review(&checked)
            .expect("raw-byte contract literal should project exactly");
        let [domain] = review.public_domains() else {
            panic!("one raw-byte domain row")
        };
        let [
            PackageReviewContractFact::Expression(PackageReviewContractExpression::Call {
                target,
                arguments,
                ..
            }),
        ] = domain.predicate_facts()
        else {
            panic!("one raw-byte predicate call")
        };
        assert_eq!(
            target.byte_sequence_predicate(),
            Some(PackageReviewByteSequencePredicate::NoNul)
        );
        let [argument] = arguments.as_slice() else {
            panic!("one exact raw-byte argument")
        };
        let row = review
            .canonical_rows()
            .expect("canonical raw-byte rows")
            .into_iter()
            .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicDomain)
            .expect("public raw-byte domain row");
        (argument.clone(), row.canonical_bytes().to_vec())
    };

    let escaped_ascii = project(r#""\x41""#);
    let direct_ascii = project(r#""A""#);
    let opaque_octet = project(r#""\xFF""#);

    assert_eq!(
        escaped_ascii.0,
        PackageReviewContractExpression::ByteSequence(vec![b'A'])
    );
    assert_eq!(escaped_ascii, direct_ascii);
    assert_eq!(
        opaque_octet.0,
        PackageReviewContractExpression::ByteSequence(vec![0xff])
    );
    assert_ne!(escaped_ascii.1, opaque_octet.1);
}

#[test]
fn review_projects_ordered_nested_array_contract_expressions() {
    let project = |literal: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub proposition matrix(values: [[i32; 2]; 2]);
pub machine consume()
requires matrix({literal})
{{ }}
"#,
            ),
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
        .expect("nested array contract fixture should check");
        project_checked_package_review(&checked)
            .expect("nested array contract expression should project in order")
    };

    let original = project("[[1, 2], [3, 4]]");
    let consume = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("consume"))
        .expect("public array consumer");
    let [contract] = consume.contracts() else {
        panic!("one array-bearing requirement")
    };
    let PackageReviewContractFact::Proposition(application) = contract.fact() else {
        panic!("one proposition application")
    };
    assert_eq!(
        application.arguments(),
        [PackageReviewContractExpression::Array(vec![
            PackageReviewContractExpression::Array(vec![
                PackageReviewContractExpression::Integer("1".to_owned()),
                PackageReviewContractExpression::Integer("2".to_owned()),
            ]),
            PackageReviewContractExpression::Array(vec![
                PackageReviewContractExpression::Integer("3".to_owned()),
                PackageReviewContractExpression::Integer("4".to_owned()),
            ]),
        ])]
    );
    let reordered = project("[[2, 1], [3, 4]]");
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        reordered.canonical_review_bytes().unwrap(),
        "array element order is semantic contract identity",
    );

    let nested = TempPackage::new();
    nested.write(
        "main.omg",
        r#"pub proposition values(items: [i32; 1]);
pub machine consume(source: [i32; 1])
requires values([source[0]])
{ }
"#,
    );
    nested.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &nested.0.join("main.omg"),
        Some("windows_x86_64"),
        package_inputs(&nested.0),
    )
    .expect("array containing an indexed expression should check");
    let nested_review = project_checked_package_review(&checked)
        .expect("array containing an indexed expression should project");
    let consume = nested_review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("consume"))
        .expect("public nested-index consumer");
    let [contract] = consume.contracts() else {
        panic!("one nested-index requirement")
    };
    let PackageReviewContractFact::Proposition(application) = contract.fact() else {
        panic!("one nested-index proposition application")
    };
    assert_eq!(
        application.arguments(),
        [PackageReviewContractExpression::Array(vec![
            PackageReviewContractExpression::Indexed {
                meaning: PackageReviewContractOperatorMeaning::Builtin,
                collection: Box::new(PackageReviewContractExpression::Parameter(0)),
                index: Box::new(PackageReviewContractExpression::Integer("0".to_owned())),
            },
        ])]
    );
}
