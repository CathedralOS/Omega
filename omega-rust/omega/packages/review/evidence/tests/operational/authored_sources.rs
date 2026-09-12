use crate::support::*;
use compiler::CheckedCompileRequest;

#[test]
fn authored_synchronous_invocations_retain_exact_review_source_spans() {
    let package = TempPackage::new();
    let source = r#"pub boundary trait Host {
    machine ping()
    reaches Host
    invokes Host;
}

pub machine dispatch(host: &mut Host)
reaches Host
invokes host;
invokes Host;
{
    host.ping();
    Host::ping();
}
"#;
    package.write("main.omg", source);
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
    })
    .expect("invocation source fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("authored invocation spans should join exact checked targets");
    let rows = review.canonical_rows().expect("invocation source rows");
    let invocation_text = |row: &package_evidence::record::PackageReviewCanonicalRow| {
        let mut text = row
            .source()
            .authored_locations()
            .expect("authored review locations")
            .iter()
            .filter(|location| {
                location.role() == PackageReviewSourceLocationRole::SynchronousInvocation
            })
            .map(|location| {
                let start = usize::try_from(location.start_byte()).unwrap();
                let end = usize::try_from(location.end_byte()).unwrap();
                source[start..end].to_owned()
            })
            .collect::<Vec<_>>();
        text.sort();
        text
    };

    let dispatch = rows
        .iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::Callable
                && row
                    .key_bytes()
                    .windows("dispatch".len())
                    .any(|window| window == b"dispatch")
        })
        .expect("dispatch callable row");
    assert_eq!(invocation_text(dispatch), ["Host", "host"]);

    let host = rows
        .iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::PublicTrait
                && row
                    .key_bytes()
                    .windows("Host".len())
                    .any(|window| window == b"Host")
        })
        .expect("Host trait row");
    assert_eq!(invocation_text(host), ["Host"]);

    let recovered = decode_package_review_canonical_row(
        &encode_package_review_canonical_row(dispatch).expect("encode invocation source row"),
    )
    .expect("recover invocation source row");
    assert!(
        recovered
            .source()
            .authored_locations()
            .is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::SynchronousInvocation
                })
            })
    );
}

#[test]
fn authored_service_reaches_retain_exact_review_sources_and_empty_ceiling_presence() {
    let package = TempPackage::new();
    let source = r#"pub boundary trait Parent {
    machine parent() reaches Parent;
}

pub boundary trait Child: Parent {
    machine ping() reaches Child + Child;
}

pub boundary trait Other { machine ping() reaches Other; }

pub machine dispatch()
reaches Child + Child
invokes Child;
{
    Child::ping();
}

pub machine sealed()
reaches
{ }

machine private_sealed()
reaches
{ }

pub machine invoke_only()
invokes Child;
{
    Child::ping();
}

machine inferred() { }

pub machine apply<machine Work>()
where machine Work()
    reaches Child;
{ }
"#;
    package.write("main.omg", source);
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
    })
    .expect("service-reach source fixture should check");
    let private_sealed = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "private_sealed")
        .expect("private sealed machine");
    let sealed_fact = checked
        .facts
        .service_reaches
        .for_machine(private_sealed.symbol)
        .expect("private sealed checked service-reach fact");
    assert_eq!(
        sealed_fact.interface,
        language_semantics::ServiceReachInterface::PublishedCeiling(
            language_semantics::ServiceReachRowTable::EMPTY_ROW,
        ),
        "an authored empty private body ceiling must not collapse into inference",
    );
    let inferred = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inferred")
        .expect("inferred machine");
    assert_eq!(
        checked
            .facts
            .service_reaches
            .for_machine(inferred.symbol)
            .expect("inferred checked service-reach fact")
            .interface,
        language_semantics::ServiceReachInterface::InternalInferred,
    );

    let review = project_checked_package_review(&checked)
        .expect("authored service-reach spans should join exact checked rows");
    let rows = review.canonical_rows().expect("service-reach source rows");
    let reach_text = |row: &package_evidence::record::PackageReviewCanonicalRow| {
        let mut text = row
            .source()
            .authored_locations()
            .expect("authored review locations")
            .iter()
            .filter(|location| location.role() == PackageReviewSourceLocationRole::ServiceReach)
            .map(|location| {
                let start = usize::try_from(location.start_byte()).unwrap();
                let end = usize::try_from(location.end_byte()).unwrap();
                source[start..end].to_owned()
            })
            .collect::<Vec<_>>();
        text.sort();
        text
    };
    let callable = |name: &str| {
        rows.iter()
            .find(|row| {
                row.kind() == PackageReviewCanonicalRowKind::Callable
                    && row
                        .key_bytes()
                        .windows(name.len())
                        .any(|window| window == name.as_bytes())
            })
            .unwrap_or_else(|| panic!("{name} callable row"))
    };
    assert_eq!(reach_text(callable("dispatch")), ["Child", "Child"]);
    assert_eq!(reach_text(callable("sealed")), ["reaches"]);
    assert_eq!(reach_text(callable("apply")), ["Child"]);
    assert!(reach_text(callable("invoke_only")).is_empty());
    assert!(
        !reach_text(callable("dispatch"))
            .iter()
            .any(|name| name == "Parent"),
        "parent closure must not invent an authored parent occurrence",
    );
    let child_trait = rows
        .iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::PublicTrait
                && row
                    .key_bytes()
                    .windows("Child".len())
                    .any(|window| window == b"Child")
        })
        .expect("Child public trait row");
    assert_eq!(reach_text(child_trait), ["Child", "Child"]);

    let dispatch = callable("dispatch");
    let recovered = decode_package_review_canonical_row(
        &encode_package_review_canonical_row(dispatch).expect("encode service-reach source row"),
    )
    .expect("recover service-reach source row");
    assert!(
        recovered
            .source()
            .authored_locations()
            .is_some_and(|locations| locations.iter().any(|location| {
                location.role() == PackageReviewSourceLocationRole::ServiceReach
            }))
    );
}

#[test]
fn propagated_public_service_reach_changes_review_without_inventing_authored_locations() {
    let mut public_rows = Vec::new();
    for service in ["Host", "Other"] {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                "pub boundary trait Host {{ machine ping() reaches Host; }}\n\
                 pub boundary trait Other {{ machine ping() reaches Other; }}\n\
                 machine helper() reaches {service} {{ }}\n\
                 pub machine forward() {{ helper(); }}\n",
            ),
        );
        package.write(
            "build.omg",
            "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        let checked = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
        })
        .expect("public wrapper should propagate its private helper's reach");
        let review = project_checked_package_review(&checked)
            .expect("propagated public summary should retain source custody");
        let forward = review
            .callables()
            .iter()
            .find(|callable| callable.identity().path() == "forward")
            .expect("public wrapper review");
        let published = forward
            .declared_service_reach()
            .expect("public wrapper must publish a service summary");
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].path(), service);
        assert_eq!(
            published[0].owner(),
            PackageReviewNominalOwner::Package(package_identity()),
        );
        let rows = review.canonical_rows().expect("canonical wrapper review");
        let forward_row = rows
            .iter()
            .find(|row| {
                row.kind() == PackageReviewCanonicalRowKind::Callable
                    && row
                        .key_bytes()
                        .windows(7)
                        .any(|window| window == b"forward")
            })
            .expect("canonical public wrapper row");
        assert!(
            forward_row
                .source()
                .authored_locations()
                .expect("wrapper authored locations")
                .iter()
                .all(|location| location.role() != PackageReviewSourceLocationRole::ServiceReach),
            "propagated services have no authored reaches occurrence on the wrapper",
        );
        public_rows.push(forward_row.canonical_bytes().to_vec());
    }
    assert_ne!(public_rows[0], public_rows[1]);
}

#[test]
fn propagated_public_service_reach_rejects_tampered_published_row() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        "pub boundary trait Host { machine ping() reaches Host; }\n\
         machine helper() reaches Host { }\n\
         pub machine forward() { helper(); }\n",
    );
    package.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let mut checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
    })
    .expect("public wrapper tampering fixture should check");
    let forward = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("public wrapper")
        .symbol;
    let root_machines = checked.facts.service_reaches.root_machines;
    let reach = checked
        .facts
        .service_reaches
        .machines
        .span_mut_or_empty(root_machines)
        .iter_mut()
        .find(|fact| fact.machine == forward)
        .expect("public wrapper checked summary");
    reach.published_ceiling = language_semantics::ServiceReachRowTable::EMPTY_ROW;
    reach.interface = language_semantics::ServiceReachInterface::PublishedCeiling(
        language_semantics::ServiceReachRowTable::EMPTY_ROW,
    );
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("consistent empty publication must not hide the helper's service reach");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("exact checked service-reach fact")
    }));
}

#[test]
fn service_reach_review_rejects_stale_target_source_and_duplicate_custody() {
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
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
    })
    .expect("service-reach tamper fixture should check");
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
        .authored_service_reach_rows
        .iter_mut()
        .find(|row| row.owner == dispatch.symbol)
        .expect("dispatch authored reach")
        .targets[0]
        .service = other;
    let diagnostics = project_checked_package_review(&target_tamper)
        .expect_err("changed exact service target must not reuse stale normalized evidence");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("do not equal its exact normalized service-reach row")
    }));

    let mut source_tamper = checked.clone();
    source_tamper
        .typed
        .authored_service_reach_rows
        .iter_mut()
        .find(|row| row.owner == dispatch.symbol)
        .expect("dispatch authored reach")
        .targets[0]
        .source_span = source::SourceSpan::default();
    let diagnostics = project_checked_package_review(&source_tamper)
        .expect_err("missing service-reach source custody must reject review");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("source span is outside"))
    );

    let mut duplicate_tamper = checked;
    let duplicate = duplicate_tamper
        .typed
        .authored_service_reach_rows
        .iter()
        .find(|row| row.owner == dispatch.symbol)
        .expect("dispatch authored reach")
        .clone();
    duplicate_tamper
        .typed
        .authored_service_reach_rows
        .push(duplicate);
    let diagnostics = project_checked_package_review(&duplicate_tamper)
        .expect_err("duplicate service-reach custody must reject review");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("authored service-reach custody rows; expected at most one")
    }));
}
