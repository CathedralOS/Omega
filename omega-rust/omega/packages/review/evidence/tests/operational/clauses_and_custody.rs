use crate::support::*;

#[test]
fn authored_operational_clauses_retain_sources_and_published_ceilings() {
    let package = TempPackage::new();
    let source = r#"pub boundary trait Worker {
    machine work()
    suspends;
    blocks;
}

pub machine operate()
suspends;
blocks;
{ }

pub machine quiet() { }

pub machine apply<machine Work>()
where machine Work()
    suspends;
    blocks;
{ }
"#;
    package.write("main.omg", source);
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
    .expect("operational source fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("authored operational spans should join exact checked rows");
    let operate = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "operate")
        .expect("operate callable");
    assert!(operate.checked_may_suspend());
    assert!(operate.checked_may_block());
    let quiet = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "quiet")
        .expect("quiet callable");
    assert!(!quiet.checked_may_suspend());
    assert!(!quiet.checked_may_block());

    let rows = review.canonical_rows().expect("operational source rows");
    let row = |kind: PackageReviewCanonicalRowKind, name: &str| {
        rows.iter()
            .find(|row| {
                row.kind() == kind
                    && row
                        .key_bytes()
                        .windows(name.len())
                        .any(|window| window == name.as_bytes())
            })
            .unwrap_or_else(|| panic!("{name} review row"))
    };
    let role_text = |row: &package_evidence::record::PackageReviewCanonicalRow,
                     role: PackageReviewSourceLocationRole| {
        row.source()
            .authored_locations()
            .expect("authored review locations")
            .iter()
            .filter(|location| location.role() == role)
            .map(|location| {
                let start = usize::try_from(location.start_byte()).unwrap();
                let end = usize::try_from(location.end_byte()).unwrap();
                source[start..end].to_owned()
            })
            .collect::<Vec<_>>()
    };
    for reviewed_row in [
        row(PackageReviewCanonicalRowKind::Callable, "operate"),
        row(PackageReviewCanonicalRowKind::Callable, "apply"),
        row(PackageReviewCanonicalRowKind::PublicTrait, "Worker"),
    ] {
        assert_eq!(
            role_text(reviewed_row, PackageReviewSourceLocationRole::Suspension),
            ["suspends"]
        );
        assert_eq!(
            role_text(reviewed_row, PackageReviewSourceLocationRole::Blocking),
            ["blocks"]
        );
    }

    let operate_row = row(PackageReviewCanonicalRowKind::Callable, "operate");
    let recovered = decode_package_review_canonical_row(
        &encode_package_review_canonical_row(operate_row).expect("encode operational source row"),
    )
    .expect("recover operational source row");
    assert!(
        recovered
            .source()
            .authored_locations()
            .is_some_and(|locations| {
                locations
                    .iter()
                    .any(|location| location.role() == PackageReviewSourceLocationRole::Suspension)
                    && locations.iter().any(|location| {
                        location.role() == PackageReviewSourceLocationRole::Blocking
                    })
            })
    );
}

#[test]
fn operational_review_rejects_missing_invalid_and_stale_source_custody() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        "pub machine operate()\nsuspends;\nblocks;\n{ }\n",
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
    .expect("operational tamper fixture should check");
    let machine_index = checked
        .machines()
        .iter()
        .position(|machine| machine.name.as_str() == "operate")
        .expect("operate machine");
    let symbol = checked.machines()[machine_index].symbol;

    let mut missing = checked.clone();
    missing.typed.machines_mut()[machine_index]
        .suspends_keyword_source_spans
        .clear();
    let diagnostics = project_checked_package_review(&missing)
        .expect_err("authored suspension without source custody must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("contradictory authored `suspends` source custody")
    }));

    let mut invalid = checked.clone();
    invalid.typed.machines_mut()[machine_index].blocks_keyword_source_spans[0] =
        source::SourceSpan::default();
    let diagnostics = project_checked_package_review(&invalid)
        .expect_err("invalid blocking source custody must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("source span is outside"))
    );

    let mut stale = checked;
    stale
        .facts
        .suspensions
        .machines
        .iter_mut()
        .find(|fact| fact.machine == symbol)
        .expect("operate suspension fact")
        .plan
        .interface = language_semantics::SuspensionInterface::InternalInferred;
    let diagnostics = project_checked_package_review(&stale)
        .expect_err("stale checked suspension interface must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("has no published suspension ceiling")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn invocation_review_rejects_target_or_source_provenance_tamper() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary trait Host { machine ping() reaches Host; }
pub boundary trait Other { machine ping() reaches Other; }
pub machine dispatch()
reaches Host
invokes Host;
{
    Host::ping();
}
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
    .expect("invocation tamper fixture should check");
    let dispatch = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "dispatch")
        .expect("dispatch machine")
        .clone();
    let other = checked
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Other")
        .expect("Other service")
        .symbol;

    let mut target_tamper = checked.clone();
    target_tamper
        .typed
        .signature_invokes
        .span_mut_or_empty(dispatch.invokes)[0]
        .target = typed_trees::signature::AuthoredInvocationTarget::Service(other);
    let diagnostics = project_checked_package_review(&target_tamper)
        .expect_err("changed exact invocation target must not reuse stale checked evidence");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("do not equal its exact checked published ceiling")
    }));

    let mut source_tamper = checked;
    source_tamper
        .typed
        .signature_invokes
        .span_mut_or_empty(dispatch.invokes)[0]
        .source_span = source::SourceSpan::default();
    let diagnostics = project_checked_package_review(&source_tamper)
        .expect_err("missing invocation source custody must reject review");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("source span is outside") }),
        "unexpected source-tamper diagnostics: {diagnostics:#?}"
    );
}
