mod support;

use support::*;

#[test]
fn public_machine_visibility_survives_checked_compilation_and_strict_empty_contracts() {
    let package = TempPackage::new();
    package.write("main.omg", "pub machine Package::entry() { }\n");
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
    .expect("public machine should check");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Package::entry")
        .expect("checked public machine");
    assert!(machine.is_public);
    assert_eq!(
        machine.supply_mode,
        psi_language_semantics::MachineSupplyMode::CheckedBody
    );

    let service = checked
        .facts
        .service_reaches
        .for_machine(machine.symbol)
        .expect("checked service contract");
    assert!(matches!(
        service.interface,
        psi_language_semantics::ServiceReachInterface::PublishedCeiling(_)
    ));
    let invocation = checked
        .facts
        .synchronous_invocations
        .for_machine(machine.symbol)
        .expect("checked invocation contract");
    assert_eq!(
        invocation.interface,
        psi_language_semantics::SynchronousInvocationInterface::PublishedCeiling
    );
    assert!(matches!(
        checked
            .facts
            .suspensions
            .for_machine(machine.symbol)
            .expect("checked suspension contract")
            .interface,
        psi_language_semantics::SuspensionInterface::PublishedMaySuspend(false)
    ));
    assert!(matches!(
        checked
            .facts
            .blocking
            .for_machine(machine.symbol)
            .expect("checked blocking contract")
            .interface,
        psi_language_semantics::BlockingInterface::PublishedMayBlock(false)
    ));
    assert_eq!(
        checked
            .facts
            .contract_plans
            .for_machine(machine.symbol)
            .expect("checked contract")
            .crash
            .interface(),
        psi_checked_trees::CrashInterface::PublishedCeiling
    );
}

#[test]
fn public_machine_cannot_hide_realized_reach_invocation_or_operational_effects() {
    let cases = [
        (
            "invocation",
            r#"pub boundary trait Handler { machine handle(); }
pub machine public_api(handler: &mut Handler) { handler.handle(); }
"#,
            &["omits `invokes handler;`"][..],
        ),
        (
            "operational",
            r#"pub boundary trait Waiting { machine wait() reaches Waiting suspends; blocks; }
pub machine public_api(waiting: &mut Waiting)
reaches Waiting
invokes waiting;
{
    suspend block waiting.wait();
}
"#,
            &["omits `suspends;`", "omits `blocks;`"][..],
        ),
        (
            "crash",
            r#"pub machine public_api() { crash Abort; }
"#,
            &["crash"][..],
        ),
    ];

    for (label, source, expected_messages) in cases {
        let package = TempPackage::new();
        package.write("main.omg", source);
        let diagnostics = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            None,
            package_inputs(&package.0),
        )
        .unwrap_err();
        for expected in expected_messages {
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "{label} omission should mention `{expected}`: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn exact_synchronous_invocations_change_comparison_encoding() {
    let quiet = TempPackage::new();
    let invoking = TempPackage::new();
    quiet.write(
        "main.omg",
        r#"pub boundary trait Handler { machine handle(); }
pub boundary trait Host { machine ping() reaches Host; }
pub machine dispatch(handler: &mut Handler)
reaches Host
invokes handler;
invokes Host;
{ }
"#,
    );
    invoking.write(
        "main.omg",
        r#"pub boundary trait Handler { machine handle(); }
pub boundary trait Host { machine ping() reaches Host; }
pub machine dispatch(handler: &mut Handler)
invokes handler;
invokes Host;
{
    handler.handle();
    Host::ping();
}
"#,
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    quiet.write("build.omg", build);
    invoking.write("build.omg", build);

    let compile = |package: &TempPackage| {
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("invocation comparison fixture should check")
    };
    let quiet = project_checked_package_review(&compile(&quiet)).expect("quiet review");
    let invoking = project_checked_package_review(&compile(&invoking)).expect("invoking review");
    let dispatch = invoking
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("public dispatch row");
    let quiet_dispatch = quiet
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("quiet public dispatch row");
    let declared = dispatch
        .declared_synchronous_invocations()
        .expect("published invocation ceiling");
    assert_eq!(declared.len(), 2);
    assert_eq!(
        declared[0],
        PackageReviewSynchronousInvocation::Parameter(0)
    );
    let PackageReviewSynchronousInvocation::Service(service) = &declared[1] else {
        panic!("second exact invocation should be a service identity")
    };
    assert_eq!(service.path(), "Host");
    assert_eq!(
        service.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(
        quiet_dispatch.declared_synchronous_invocations(),
        Some(declared)
    );
    assert!(quiet_dispatch.realized_synchronous_invocations().is_empty());
    assert_eq!(quiet_dispatch.contracts(), dispatch.contracts());
    assert_eq!(dispatch.realized_synchronous_invocations(), declared);
    assert_ne!(
        quiet.canonical_review_bytes().expect("quiet encoding"),
        invoking
            .canonical_review_bytes()
            .expect("invoking encoding")
    );
}

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
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("invocation source fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("authored invocation spans should join exact checked targets");
    let rows = review.canonical_rows().expect("invocation source rows");
    let invocation_text = |row: &omega_package_review::PackageReviewCanonicalRow| {
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
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
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
        psi_language_semantics::ServiceReachInterface::PublishedCeiling(
            psi_language_semantics::ServiceReachRowTable::EMPTY_ROW,
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
        psi_language_semantics::ServiceReachInterface::InternalInferred,
    );

    let review = project_checked_package_review(&checked)
        .expect("authored service-reach spans should join exact checked rows");
    let rows = review.canonical_rows().expect("service-reach source rows");
    let reach_text = |row: &omega_package_review::PackageReviewCanonicalRow| {
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
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
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
        .source_span = psi_source::SourceSpan::default();
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
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
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
    let role_text = |row: &omega_package_review::PackageReviewCanonicalRow,
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
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
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
        psi_source::SourceSpan::default();
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
        .interface = psi_language_semantics::SuspensionInterface::InternalInferred;
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
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
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
        .target = psi_typed_trees::signature::AuthoredInvocationTarget::Service(other);
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
        .source_span = psi_source::SourceSpan::default();
    let diagnostics = project_checked_package_review(&source_tamper)
        .expect_err("missing invocation source custody must reject review");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("source span is outside") }),
        "unexpected source-tamper diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn review_rejects_target_free_and_standalone_checked_programs() {
    let package = TempPackage::new();
    package.write("main.omg", "machine local() { }\n");

    let target_free = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        None,
        package_inputs(&package.0),
    )
    .expect("target-free package fixture should check");
    let diagnostics = project_checked_package_review(&target_free)
        .expect_err("review must require an explicit target");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires one explicit target selection")
    }));

    let standalone = omega_compiler::compile_to_checked(&package.0.join("main.omg"), None)
        .expect("standalone fixture should check");
    let diagnostics = project_checked_package_review(&standalone)
        .expect_err("review must require package-aware compilation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires package-aware checked compilation")
    }));
}

#[test]
fn review_distinguishes_profiles_that_share_a_native_target() {
    let package = TempPackage::new();
    package.write("main.omg", "machine local() { }\n");
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target uefi_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let windows = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("Windows review fixture should check");
    let uefi = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("uefi_x64"),
        package_inputs(&package.0),
    )
    .expect("UEFI review fixture should check");

    assert_eq!(
        windows.selected_native_target(),
        uefi.selected_native_target()
    );
    let windows = project_checked_package_review(&windows).expect("Windows review projection");
    let uefi = project_checked_package_review(&uefi).expect("UEFI review projection");
    assert_eq!(windows.target(), omega_target::TargetProfile::WindowsX64);
    assert_eq!(uefi.target(), omega_target::TargetProfile::UefiX64);
    assert_ne!(windows.target(), uefi.target());
    assert_ne!(
        windows.canonical_review_bytes().expect("Windows encoding"),
        uefi.canonical_review_bytes().expect("UEFI encoding"),
    );
}

#[test]
fn review_encoding_ignores_unreviewed_arena_insertion_order() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    first.write("main.omg", "boundary machine host_ping();\n");
    second.write(
        "main.omg",
        "machine unrelated() { }\nboundary machine host_ping();\n",
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let compile = |package: &TempPackage| {
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("arena-order fixture should check")
    };
    let first = project_checked_package_review(&compile(&first))
        .expect("first arena-order review")
        .canonical_review_bytes()
        .expect("first arena-order encoding");
    let second = project_checked_package_review(&compile(&second))
        .expect("second arena-order review")
        .canonical_review_bytes()
        .expect("second arena-order encoding");

    assert_eq!(first, second);
}
