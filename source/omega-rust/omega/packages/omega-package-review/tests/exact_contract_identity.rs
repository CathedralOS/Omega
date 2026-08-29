mod support;

use support::*;

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
            "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
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
            "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("compiler builtin-function contract should check");
        project_checked_package_review(&checked)
            .expect("compiler builtin function should have exact review identity")
    };

    let mut encodings = Vec::new();
    for (expression, expected) in [
        (
            "min(left, right) == left",
            psi_symbols::BuiltinFunction::Min,
        ),
        (
            "max(left, right) == right",
            psi_symbols::BuiltinFunction::Max,
        ),
        ("sqrt(left) == right", psi_symbols::BuiltinFunction::Sqrt),
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
        "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let mut checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("builtin target-tamper fixture should check");
    let call_expression = checked
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| {
            matches!(node, psi_typed_trees::expression::ExpressionNode::Call(_))
                .then_some(expression)
        })
        .expect("builtin call expression");
    let psi_typed_trees::expression::ExpressionNode::Call(call) = checked
        .typed
        .expression_table
        .expression_mut(call_expression)
    else {
        panic!("call expression")
    };
    call.target_symbol = psi_symbols::SymbolHandle::invalid();

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
            "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
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
            r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
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
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &nested.0.join("main.omg"),
        Some("windows_x64"),
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

#[test]
fn review_projects_exact_nominal_record_and_case_constructors() {
    let project = |point_fields: &str, case: &str, case_fields: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub data Point [copy] {{ x: i32; y: i32; }}
pub data Outcome [copy] {{
    code: u64;
    case Success(value: u64);
    case Failure(value: u64);
}}
pub proposition has_point(value: Point);
pub proposition has_outcome(value: Outcome);
pub machine consume()
requires has_point(Point {{ {point_fields} }})
requires has_outcome(Outcome::{case} {{ {case_fields} }})
{{ }}
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
        .expect("nominal constructor contract fixture should check");
        project_checked_package_review(&checked)
            .expect("nominal constructors should project by exact declaration identity")
    };

    let original = project("x: 1, y: 2", "Success", "code: 3u64, value: 4u64");
    let consume = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("consume"))
        .expect("public constructor consumer");
    assert_eq!(consume.contracts().len(), 2);
    let point = consume
        .contracts()
        .iter()
        .find_map(|contract| match contract.fact() {
            PackageReviewContractFact::Proposition(application)
                if application.declaration().path() == "has_point" =>
            {
                application.arguments().first()
            }
            _ => None,
        })
        .expect("record constructor argument");
    let PackageReviewContractExpression::Constructor { data, case, fields } = point else {
        panic!("one exact record constructor")
    };
    assert_eq!(data.path(), "Point");
    assert_eq!(
        data.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert!(case.is_none());
    assert_eq!(fields.len(), 2);
    assert!(fields[0].field().path() < fields[1].field().path());

    let outcome = consume
        .contracts()
        .iter()
        .find_map(|contract| match contract.fact() {
            PackageReviewContractFact::Proposition(application)
                if application.declaration().path() == "has_outcome" =>
            {
                application.arguments().first()
            }
            _ => None,
        })
        .expect("case constructor argument");
    let PackageReviewContractExpression::Constructor {
        data,
        case: Some(case),
        fields,
    } = outcome
    else {
        panic!("one exact sum-case constructor")
    };
    assert_eq!(data.path(), "Outcome");
    assert!(case.path().contains("Success"));
    assert_eq!(fields.len(), 2, "record and selected-case payload fields");

    let reordered = project("y: 2, x: 1", "Success", "value: 4u64, code: 3u64");
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        reordered.canonical_review_bytes().unwrap(),
        "constructor field spelling order must canonicalize by exact field identity",
    );
    let changed_case = project("x: 1, y: 2", "Failure", "code: 3u64, value: 4u64");
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_case.canonical_review_bytes().unwrap(),
        "changing the exact selected case must change review identity",
    );
    let changed_value = project("x: 1, y: 2", "Success", "code: 3u64, value: 5u64");
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_value.canonical_review_bytes().unwrap(),
        "changing a constructor field value must change review identity",
    );

    let private = TempPackage::new();
    private.write(
        "main.omg",
        r#"data Hidden [copy] { value: u64; }
pub proposition hidden(value: Hidden);
pub machine consume()
requires hidden(Hidden { value: 1u64 })
{ }
"#,
    );
    private.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let diagnostics = compile_to_checked_with_packages(
        &private.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&private.0),
    )
    .expect_err("a public contract must reject a private constructor before review");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("public interface selects private data `Hidden`")
    }));
}

#[test]
fn review_projects_checked_index_and_range_contract_expressions() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub proposition selected(value: i32);
pub proposition window(values: &[i32]);
pub machine inspect(values: [i32; 2])
requires
    selected(values[0]),
    window(values[0..1])
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
    .expect("indexed public contract fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("checked index and range expressions should project");
    let inspect = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("inspect"))
        .expect("public indexed-contract callable");
    let [selected, window] = inspect.contracts() else {
        panic!("two indexed requirements")
    };

    let PackageReviewContractFact::Proposition(selected) = selected.fact() else {
        panic!("selected proposition application")
    };
    let [
        PackageReviewContractExpression::Indexed {
            meaning,
            collection,
            index,
        },
    ] = selected.arguments()
    else {
        panic!("selected argument is one indexed expression")
    };
    assert_eq!(*meaning, PackageReviewContractOperatorMeaning::Builtin);
    assert_eq!(**collection, PackageReviewContractExpression::Parameter(0));
    assert_eq!(
        **index,
        PackageReviewContractExpression::Integer("0".to_owned())
    );

    let PackageReviewContractFact::Proposition(window) = window.fact() else {
        panic!("window proposition application")
    };
    let [
        PackageReviewContractExpression::Indexed {
            meaning,
            collection,
            index,
        },
    ] = window.arguments()
    else {
        panic!("window argument is one indexed expression")
    };
    assert_eq!(*meaning, PackageReviewContractOperatorMeaning::Builtin);
    assert_eq!(**collection, PackageReviewContractExpression::Parameter(0));
    assert_eq!(
        **index,
        PackageReviewContractExpression::Range {
            start: Some(Box::new(PackageReviewContractExpression::Integer(
                "0".to_owned(),
            ))),
            end: Some(Box::new(PackageReviewContractExpression::Integer(
                "1".to_owned(),
            ))),
            end_inclusive: false,
        }
    );
    let baseline_bytes = review
        .canonical_review_bytes()
        .expect("indexed contract review must encode canonically");

    package.write(
        "main.omg",
        r#"pub proposition selected(value: i32);
pub proposition window(values: &[i32]);
pub machine inspect(values: [i32; 2])
requires
    selected(values[1]),
    window(values[0..=1])
{ }
"#,
    );
    let changed = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("changed indexed public contract fixture should check");
    let changed_bytes = project_checked_package_review(&changed)
        .expect("changed checked index and range expressions should project")
        .canonical_review_bytes()
        .expect("changed indexed contract review must encode canonically");
    assert_ne!(baseline_bytes, changed_bytes);
}

#[test]
fn review_projects_exact_zero_value_targets_in_public_contracts() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let source = |binder: &str, family: &str| {
        format!(
            r#"pub data Optional<Element> {{ case #0 None; }}
pub data Alternate<Element> {{ case #0 None; }}
pub proposition zero_is_none<{binder}>() =
    zero_value<{family}<{binder}>>() == zero_value<{family}<{binder}>>();
"#
        )
    };
    let project = |source: String| {
        package.write("main.omg", &source);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("zero-value public proposition should check");
        project_checked_package_review(&checked)
            .expect("zero-value public proposition should project exactly")
    };

    let first = project(source("Item", "Optional"));
    let proposition = first
        .public_propositions()
        .iter()
        .find(|shape| shape.identity().path() == "zero_is_none")
        .expect("public zero-value proposition row");
    let PackageReviewPublicPropositionBody::Transparent(PackageReviewContractFact::Expression(
        PackageReviewContractExpression::Binary { left, .. },
    )) = proposition.body()
    else {
        panic!("one transparent zero-value equality")
    };
    let PackageReviewContractExpression::ZeroValue(target_type) = left.as_ref() else {
        panic!("the proof-only observation retains its exact target type")
    };
    assert!(target_type.canonical().contains("Optional"));
    let first_bytes = first.canonical_review_bytes().unwrap();

    let renamed = project(source("Value", "Optional"));
    assert_eq!(first_bytes, renamed.canonical_review_bytes().unwrap());
    let changed = project(source("Item", "Alternate"));
    assert_ne!(first_bytes, changed.canonical_review_bytes().unwrap());

    package.write(
        "main.omg",
        r#"data Hidden {}
pub proposition hidden_zero() =
    zero_value<Hidden>() == zero_value<Hidden>();
"#,
    );
    let diagnostics = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect_err("a public zero-value target must not expose a private data declaration");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("public interface selects private data `Hidden`")
    }));
}

#[test]
fn package_callable_wins_over_compiler_byte_predicate_spelling_in_review() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub machine valid_utf8(value: &[u8]) -> bool { true }
pub domain [u8]::ReviewedBytes
requires
    valid_utf8(self);
"#,
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
    .expect("package callable lookalike should check as an ordinary call");
    let review = project_checked_package_review(&checked)
        .expect("package callable lookalike should retain nominal identity");
    let [domain] = review.public_domains() else {
        panic!("one byte-domain row")
    };
    let [
        PackageReviewContractFact::Expression(PackageReviewContractExpression::Call {
            target, ..
        }),
    ] = domain.predicate_facts()
    else {
        panic!("one nominal predicate call")
    };
    let nominal = target
        .nominal()
        .expect("package declaration must remain nominal");
    assert_eq!(nominal.path(), "valid_utf8::entry");
    assert_eq!(
        nominal.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
}

#[test]
fn public_domain_predicate_review_rejects_checked_owner_and_dependency_spoofs() {
    let compile = || {
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
            "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public domain spoof fixture should check")
    };
    let assert_rejects = |checked: &_, expected: &str| {
        let diagnostics = project_checked_package_review(checked)
            .expect_err("spoofed checked domain ownership must reject");
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
        .domain_definition_facts
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("domain ownership record");
    assert!(
        missing_owner
            .facts
            .semantic
            .domain_definition_facts
            .free(owner)
    );
    assert_rejects(&missing_owner, "0 exact checked ownership records");

    let mut duplicate_owner = compile();
    let owner = duplicate_owner
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.clone())
        .expect("domain ownership record");
    duplicate_owner
        .facts
        .semantic
        .domain_definition_facts
        .append(owner);
    assert_rejects(&duplicate_owner, "2 exact checked ownership records");

    let mut wrong_origin = compile();
    let semantic_fact = wrong_origin
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.semantic_fact)
        .expect("domain semantic fact");
    wrong_origin
        .facts
        .semantic
        .facts
        .get_mut(semantic_fact)
        .origin = psi_facts::FactOrigin::Unknown;
    assert_rejects(&wrong_origin, "0 exact checked definition rows");

    let mut false_evidence = compile();
    let semantic_fact = false_evidence
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.semantic_fact)
        .expect("domain semantic fact");
    false_evidence
        .facts
        .semantic
        .facts
        .get_mut(semantic_fact)
        .evidence
        .receipt_identity = 1;
    assert_rejects(&false_evidence, "0 exact checked definition rows");

    let mut missing_dependency = compile();
    let owner = missing_dependency
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("domain ownership record");
    missing_dependency
        .facts
        .semantic
        .domain_definition_facts
        .get_mut(owner)
        .dependencies
        .clear();
    assert_rejects(&missing_dependency, "0 exact checked dependency records");

    let mut duplicate_dependency = compile();
    let owner = duplicate_dependency
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("domain ownership record");
    let dependency = duplicate_dependency
        .facts
        .semantic
        .domain_definition_facts
        .get(owner)
        .dependencies[0];
    duplicate_dependency
        .facts
        .semantic
        .domain_definition_facts
        .get_mut(owner)
        .dependencies
        .push(dependency);
    assert_rejects(&duplicate_dependency, "2 exact checked dependency records");

    let mut wrong_member = compile();
    let dependency = wrong_member
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .next()
        .and_then(|(_, record)| record.dependencies.first().copied())
        .expect("domain member dependency");
    let segment = wrong_member
        .facts
        .semantic
        .places
        .get(dependency.place)
        .segments
        .start();
    wrong_member
        .facts
        .semantic
        .place_segments
        .get_mut(segment)
        .clone_from(&psi_facts::PlaceSegment::Field {
            symbol: psi_symbols::SymbolHandle::invalid(),
        });
    assert_rejects(&wrong_member, "0 exact checked dependency records");
}

#[test]
fn review_projects_public_domain_membership_predicates_and_rejects_private_targets() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Packet { value: u32; }
pub domain Packet::Base;
pub domain Packet::Ready
    requires self in Packet::Base;
"#,
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
    .expect("public domain membership fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public domain membership review should close");
    let ready = review
        .public_domains()
        .iter()
        .find(|domain| domain.identity().path() == "Packet::Ready")
        .expect("ready domain row");
    let [PackageReviewContractFact::Membership { value, domain }] = ready.predicate_facts() else {
        panic!("one exact membership predicate")
    };
    assert_eq!(value, &PackageReviewContractExpression::DomainSubject);
    assert_eq!(domain.path(), "Packet::Base");

    let private = TempPackage::new();
    private.write(
        "main.omg",
        r#"pub data Packet { value: u32; }
domain Packet::Base;
pub domain Packet::Ready
    requires self in Packet::Base;
"#,
    );
    private.write(
        "build.omg",
        "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let diagnostics = compile_to_checked_with_packages(
        &private.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&private.0),
    )
    .expect_err("ordinary visibility must reject a private domain in a public predicate");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private domain `Packet::Base`")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn public_domain_predicate_fact_order_is_canonical_but_content_changes_encoding() {
    let first = TempPackage::new();
    let reordered = TempPackage::new();
    let changed = TempPackage::new();
    let source = |facts: &str| {
        format!(
            "pub data Packet {{ value: u32; }}\npub domain Packet::Ready\nrequires\n    {facts}\n"
        )
    };
    first.write("main.omg", &source("self.value == 0; self.value <= 1;"));
    reordered.write("main.omg", &source("self.value <= 1; self.value == 0;"));
    changed.write("main.omg", &source("self.value == 0; self.value <= 2;"));
    let build = "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n";
    first.write("build.omg", build);
    reordered.write("build.omg", build);
    changed.write("build.omg", build);

    let encode = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("multi-fact public domain fixture should check");
        project_checked_package_review(&checked)
            .expect("multi-fact public domain review should close")
            .canonical_review_bytes()
            .expect("multi-fact public domain encoding")
    };
    assert_eq!(encode(&first), encode(&reordered));
    assert_ne!(encode(&first), encode(&changed));
}

#[test]
fn public_api_rows_retain_exact_nested_authored_extents() {
    let package = TempPackage::new();
    let source = r#"pub data Ledger
where
    count <= len,
{
    len: u32;
    count: u32;
}
pub domain Ledger::Ready
requires
    self.count <= self.len;
pub machine identity(input: u64) -> u64 { input }
pub operator Ledger::project(record: Ledger) -> u32;
pub trait Bounds {
    machine clamp(value: u64) -> u64
    requires value >= 1
    ensures result >= value;
}
"#;
    package.write("main.omg", source);
    package.write(
        "build.omg",
        "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("proof-fact source fixture should check");
    let rows = project_checked_package_review(&checked)
        .expect("proof-fact source fixture should project")
        .canonical_rows()
        .expect("proof-fact canonical rows");

    let role_slices = |kind, role| {
        rows.iter()
            .filter(|row| row.kind() == kind)
            .flat_map(|row| row.source().authored_locations().unwrap_or_default())
            .filter(|location| location.role() == role && location.relative_path() == "main.omg")
            .map(|location| {
                &source[usize::try_from(location.start_byte()).unwrap()
                    ..usize::try_from(location.end_byte()).unwrap()]
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        role_slices(
            PackageReviewCanonicalRowKind::PublicData,
            PackageReviewSourceLocationRole::ProofFact,
        ),
        ["count <= len"]
    );
    assert_eq!(
        role_slices(
            PackageReviewCanonicalRowKind::PublicDomain,
            PackageReviewSourceLocationRole::ProofFact,
        ),
        ["self.count <= self.len"]
    );
    assert_eq!(
        role_slices(
            PackageReviewCanonicalRowKind::PublicTrait,
            PackageReviewSourceLocationRole::ProofFact,
        ),
        ["value >= 1", "result >= value"]
    );
    assert_eq!(
        role_slices(
            PackageReviewCanonicalRowKind::PublicData,
            PackageReviewSourceLocationRole::DataMember,
        ),
        ["len", "count"]
    );
    assert_eq!(
        role_slices(
            PackageReviewCanonicalRowKind::PublicTrait,
            PackageReviewSourceLocationRole::TraitRequirement,
        ),
        ["clamp"]
    );
    assert_eq!(
        role_slices(
            PackageReviewCanonicalRowKind::Callable,
            PackageReviewSourceLocationRole::CallableParameter,
        ),
        ["input"]
    );
    assert_eq!(
        role_slices(
            PackageReviewCanonicalRowKind::PublicOperator,
            PackageReviewSourceLocationRole::CallableParameter,
        ),
        ["record"]
    );
    assert_eq!(
        role_slices(
            PackageReviewCanonicalRowKind::PublicTrait,
            PackageReviewSourceLocationRole::CallableParameter,
        ),
        ["value"]
    );
}

#[test]
fn review_projects_callable_domain_predicates_through_exact_checked_selection() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Reading {
    value: i64;
    minimum: i64;
    maximum: i64;
}
pub machine within_calibration(reading: Reading) -> bool {
    reading.value >= reading.minimum && reading.value <= reading.maximum
}
pub domain Reading::Calibrated
requires
    within_calibration(self);
"#,
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
    .expect("callable domain predicate fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("simple checked callable predicates have an exact review row");
    let [domain] = review.public_domains() else {
        panic!("one public domain row")
    };
    let [
        PackageReviewContractFact::Expression(PackageReviewContractExpression::Call {
            receiver,
            target,
            static_arguments,
            arguments,
        }),
    ] = domain.predicate_facts()
    else {
        panic!("one callable domain predicate")
    };
    assert!(receiver.is_none());
    assert_eq!(
        target.nominal().expect("ordinary callable target").path(),
        "within_calibration::entry"
    );
    assert!(static_arguments.is_empty());
    assert_eq!(arguments, &[PackageReviewContractExpression::DomainSubject]);
}

#[test]
fn callable_domain_predicate_review_rejects_checked_target_spoof() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Reading { value: i64; }
pub machine is_zero(reading: Reading) -> bool { reading.value == 0 }
pub domain Reading::Zero requires is_zero(self);
"#,
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
    .expect("callable predicate spoof fixture should check");
    let call_expression = checked
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| {
            matches!(node, psi_typed_trees::expression::ExpressionNode::Call(_))
                .then_some(expression)
        })
        .expect("domain predicate call expression");
    let psi_typed_trees::expression::ExpressionNode::Call(call) = checked
        .typed
        .expression_table
        .expression_mut(call_expression)
    else {
        panic!("call expression")
    };
    call.target_symbol = psi_symbols::SymbolHandle::invalid();

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("a typed call target cannot diverge from checked selection custody");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("target disagrees with its exact checked call-selection row")
    }));
}
