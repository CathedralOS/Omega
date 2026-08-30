use crate::support::*;

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
        "target windows_x86_64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
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
        "target windows_x86_64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
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
        "target windows_x86_64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let mut checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
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
