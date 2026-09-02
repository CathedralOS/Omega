use super::*;

#[test]
fn public_const_changes_render_as_blocking_review_conflicts() {
    let live = temp_root("public-const-live");
    let baseline_cache = temp_root("public-const-baseline");
    let candidate_cache = temp_root("public-const-candidate");
    let build_root = temp_root("public-const-build");
    let context = ExternalSourceContext::derive(b"public-const-conflict-test");

    write_package(&live, "pub const LIMIT: u64 = 4;\n");
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public const baseline");
    let baseline_reviews = compile_resolved_package_reviews(
        &baseline_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile public const baseline");

    write_package(&live, "pub const LIMIT: u64 = 5;\n");
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public const candidate");
    let candidate_reviews = compile_resolved_package_reviews(
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile public const candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare public const compatibility");
    assert_eq!(conflicts.conflict_count(), 1);
    let [package] = conflicts.packages() else {
        panic!("one changed package")
    };
    let [conflict] = package.conflicts() else {
        panic!("one changed public const row")
    };
    assert_eq!(conflict.kind(), PackageReviewCanonicalRowKind::PublicConst);
    assert_eq!(conflict.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert_eq!(
        conflict.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    assert!(conflict.is_blocking());
    let baseline_locations = conflict
        .baseline_source()
        .and_then(PackageReviewCanonicalRowSource::authored_locations)
        .expect("changed public const retains baseline source custody");
    let candidate_locations = conflict
        .candidate_source()
        .and_then(PackageReviewCanonicalRowSource::authored_locations)
        .expect("changed public const retains candidate source custody");
    for locations in [baseline_locations, candidate_locations] {
        assert!(locations.iter().any(|location| {
            location.role() == PackageReviewSourceLocationRole::ConstInitializer
                && location.relative_path() == "main.omg"
        }));
    }
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render public const conflict");
    assert!(rendered.starts_with("OMEGA_PACKAGE_CAPABILITY_CONFLICTS_V20\n"));
    assert!(rendered.contains("change changed\nkind public_const\nrisk blocking\n"));
    assert!(rendered.contains("baseline_location const_initializer package "));
    assert!(rendered.contains("candidate_location const_initializer package "));
    assert_ne!(conflict.fingerprint().digest(), [0; 32]);

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}
#[test]
fn public_operator_changes_render_as_blocking_review_conflicts() {
    let live = temp_root("public-operator-live");
    let baseline_cache = temp_root("public-operator-baseline");
    let candidate_cache = temp_root("public-operator-candidate");
    let build_root = temp_root("public-operator-build");
    let context = ExternalSourceContext::derive(b"public-operator-conflict-test");

    write_package(
        &live,
        "pub data Token [copy] { value: u64; }\npub operator < Token::less(left: Token, right: Token) -> bool;\n",
    );
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public operator baseline");
    let baseline_reviews = compile_resolved_package_reviews(
        &baseline_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile public operator baseline");

    write_package(
        &live,
        "pub data Token [copy] { value: u64; }\npub operator < Token::less(left: Token, right: Token) -> bool\nrequires true;\n",
    );
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public operator candidate");
    let candidate_reviews = compile_resolved_package_reviews(
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile public operator candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare public operator compatibility");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::PublicOperator)
        .expect("changed public operator row");
    assert_eq!(conflict.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert_eq!(
        conflict.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    assert!(conflict.is_blocking());
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render public operator conflict");
    assert!(rendered.contains("change changed\nkind public_operator\nrisk blocking\n"));
    assert!(rendered.contains("candidate_location contract_clause package "));

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}
#[test]
fn public_callable_parameter_changes_render_exact_parameter_locations() {
    let live = temp_root("public-callable-parameter-live");
    let baseline_cache = temp_root("public-callable-parameter-baseline");
    let candidate_cache = temp_root("public-callable-parameter-candidate");
    let build_root = temp_root("public-callable-parameter-build");
    let context = ExternalSourceContext::derive(b"public-callable-parameter-conflict-test");
    let baseline_source = "pub machine inspect(baseline_value: u32) -> u32 { baseline_value }\n";
    let candidate_source = "pub machine inspect(candidate_value: u32) -> u32 { candidate_value }\n";

    write_package(&live, baseline_source);
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public callable parameter baseline");
    let baseline_reviews = compile_resolved_package_reviews(
        &baseline_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile public callable parameter baseline");

    write_package(&live, candidate_source);
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public callable parameter candidate");
    let candidate_reviews = compile_resolved_package_reviews(
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile public callable parameter candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare public callable parameter rows");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::Callable)
        .expect("changed public callable row");
    let expected_locations = [
        (
            conflict.baseline_source(),
            baseline_source,
            "baseline_value",
        ),
        (
            conflict.candidate_source(),
            candidate_source,
            "candidate_value",
        ),
    ];
    for (source, package_source, parameter) in expected_locations {
        let start = u64::try_from(
            package_source
                .find(parameter)
                .expect("parameter identifier in package source"),
        )
        .expect("parameter start fits review coordinate");
        let end = start
            + u64::try_from(parameter.len()).expect("parameter length fits review coordinate");
        let location = source
            .and_then(PackageReviewCanonicalRowSource::authored_locations)
            .expect("public callable source locations")
            .iter()
            .find(|location| location.role() == PackageReviewSourceLocationRole::CallableParameter)
            .expect("exact callable parameter source location");
        assert_eq!(location.relative_path(), "main.omg");
        assert_eq!(location.start_byte(), start);
        assert_eq!(location.end_byte(), end);
    }

    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render public callable parameter conflict");
    assert!(rendered.starts_with("OMEGA_PACKAGE_CAPABILITY_CONFLICTS_V20\n"));
    for (label, package_source, parameter) in [
        ("baseline", baseline_source, "baseline_value"),
        ("candidate", candidate_source, "candidate_value"),
    ] {
        let start = u64::try_from(
            package_source
                .find(parameter)
                .expect("rendered parameter identifier in package source"),
        )
        .expect("rendered parameter start fits review coordinate");
        let end = start
            + u64::try_from(parameter.len()).expect("parameter length fits review coordinate");
        let line = rendered
            .lines()
            .find(|line| line.starts_with(&format!("{label}_location callable_parameter package ")))
            .expect("rendered callable parameter location");
        assert!(line.ends_with(&format!(" {start} {end} \"main.omg\"")));
    }

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn callable_changes_render_exact_checked_body_call_locations() {
    let live = temp_root("body-call-live");
    let baseline_cache = temp_root("body-call-baseline");
    let candidate_cache = temp_root("body-call-candidate");
    let build_root = temp_root("body-call-build");
    let context = ExternalSourceContext::derive(b"body-call-conflict-test");
    let source = |target: &str| {
        format!(
            "machine first() {{ }}\nmachine second() {{ }}\npub machine run() {{ {target}(); }}\n"
        )
    };

    write_package(&live, &source("first"));
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve body-call baseline");
    let baseline_reviews = compile_resolved_package_reviews(
        &baseline_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile body-call baseline");

    write_package(&live, &source("second"));
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve body-call candidate");
    let candidate_reviews = compile_resolved_package_reviews(
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile body-call candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare changed checked body call");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| {
            conflict.kind() == PackageReviewCanonicalRowKind::Callable
                && conflict
                    .row_key()
                    .windows("run".len())
                    .any(|window| window == b"run")
        })
        .expect("changed run callable row");
    assert_eq!(conflict.risk(), PackageReviewCanonicalRowRisk::Blocking);
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render checked body-call conflict");
    assert!(rendered.contains("baseline_location body_call package "));
    assert!(rendered.contains("candidate_location body_call package "));

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn public_conformance_changes_render_as_blocking_review_conflicts() {
    let live = temp_root("public-conformance-live");
    let baseline_cache = temp_root("public-conformance-baseline");
    let candidate_cache = temp_root("public-conformance-candidate");
    let build_root = temp_root("public-conformance-build");
    let context = ExternalSourceContext::derive(b"public-conformance-conflict-test");

    let source = |argument: &str| {
        format!(
            r#"pub data First {{ }}
pub data Second {{ }}
pub trait Marker<Tag> {{ }}
pub Choice: First satisfies Marker<{argument}> {{ }}
"#
        )
    };
    write_package(&live, &source("First"));
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public conformance baseline");
    let baseline_reviews = compile_resolved_package_reviews(
        &baseline_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile public conformance baseline");

    write_package(&live, &source("Second"));
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public conformance candidate");
    let candidate_reviews = compile_resolved_package_reviews(
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile public conformance candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare public conformance compatibility");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("changed public conformance row");
    assert_eq!(conflict.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert_eq!(
        conflict.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    assert!(conflict.is_blocking());
    assert!(
        conflicts
            .render_bounded(1024 * 1024)
            .expect("render public conformance conflict")
            .contains("change changed\nkind public_conformance\nrisk blocking\n")
    );

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn public_trait_requirement_changes_render_exact_requirement_locations() {
    let live = temp_root("public-trait-requirement-live");
    let baseline_cache = temp_root("public-trait-requirement-baseline");
    let candidate_cache = temp_root("public-trait-requirement-candidate");
    let build_root = temp_root("public-trait-requirement-build");
    let context = ExternalSourceContext::derive(b"public-trait-requirement-conflict-test");

    let source = |parameter: &str| {
        format!(
            r#"pub trait Handler {{
    machine handle(value: {parameter}) -> u64;
}}
"#
        )
    };
    write_package(&live, &source("u32"));
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public trait requirement baseline");
    let baseline_reviews = compile_resolved_package_reviews(
        &baseline_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile public trait requirement baseline");

    write_package(&live, &source("u64"));
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public trait requirement candidate");
    let candidate_reviews = compile_resolved_package_reviews(
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile public trait requirement candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare public trait requirement rows");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::PublicTrait)
        .expect("changed public trait row");
    for source in [conflict.baseline_source(), conflict.candidate_source()] {
        assert!(
            source
                .and_then(PackageReviewCanonicalRowSource::authored_locations)
                .expect("public trait source locations")
                .iter()
                .any(|location| {
                    location.role() == PackageReviewSourceLocationRole::TraitRequirement
                        && location.relative_path() == "main.omg"
                })
        );
    }
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render public trait requirement conflict");
    assert!(rendered.starts_with("OMEGA_PACKAGE_CAPABILITY_CONFLICTS_V20\n"));
    assert!(rendered.contains("baseline_location trait_requirement package "));
    assert!(rendered.contains("candidate_location trait_requirement package "));

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn public_trait_parent_changes_render_exact_nested_review_locations() {
    let live = temp_root("public-trait-parent-live");
    let baseline_cache = temp_root("public-trait-parent-baseline");
    let candidate_cache = temp_root("public-trait-parent-candidate");
    let build_root = temp_root("public-trait-parent-build");
    let context = ExternalSourceContext::derive(b"public-trait-parent-conflict-test");

    let source = |parent: &str| {
        format!(
            r#"pub trait First {{ }}
pub trait Second {{ }}
pub trait Child: {parent} {{ }}
"#
        )
    };
    write_package(&live, &source("First"));
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public-trait baseline");
    let baseline_reviews = compile_resolved_package_reviews(
        &baseline_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile public-trait baseline");

    write_package(&live, &source("Second"));
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public-trait candidate");
    let candidate_reviews = compile_resolved_package_reviews(
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile public-trait candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare public-trait compatibility");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::PublicTrait)
        .expect("changed public-trait row");
    assert_eq!(conflict.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert_eq!(
        conflict.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render public-trait conflict");
    assert!(rendered.contains("change changed\nkind public_trait\nrisk blocking\n"));
    assert!(rendered.contains("baseline_location trait_parent package "));
    assert!(rendered.contains("candidate_location trait_parent package "));

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn public_data_shape_changes_render_exact_member_locations() {
    let live = temp_root("public-data-member-live");
    let baseline_cache = temp_root("public-data-member-baseline");
    let candidate_cache = temp_root("public-data-member-candidate");
    let build_root = temp_root("public-data-member-build");
    let context = ExternalSourceContext::derive(b"public-data-member-conflict-test");

    write_package(&live, "pub data Packet { value: u32; }\n");
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public data baseline");
    let baseline_reviews = compile_resolved_package_reviews(
        &baseline_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile public data baseline");

    write_package(&live, "pub data Packet { value: u64; }\n");
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public data candidate");
    let candidate_reviews = compile_resolved_package_reviews(
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile public data candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare public data rows");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::PublicData)
        .expect("changed public data row");
    for source in [conflict.baseline_source(), conflict.candidate_source()] {
        assert!(
            source
                .and_then(PackageReviewCanonicalRowSource::authored_locations)
                .expect("public data source locations")
                .iter()
                .any(|location| {
                    location.role() == PackageReviewSourceLocationRole::DataMember
                        && location.relative_path() == "main.omg"
                })
        );
    }
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render public data member conflict");
    assert!(rendered.starts_with("OMEGA_PACKAGE_CAPABILITY_CONFLICTS_V20\n"));
    assert!(rendered.contains("baseline_location data_member package "));
    assert!(rendered.contains("candidate_location data_member package "));

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}
