use super::*;

#[test]
fn invocation_changes_render_exact_authored_target_locations() {
    let live = temp_root("invocation-location-live");
    let baseline_cache = temp_root("invocation-location-baseline");
    let candidate_cache = temp_root("invocation-location-candidate");
    let build_root = temp_root("invocation-location-build");
    let context = ExternalSourceContext::derive(b"invocation-location-conflict-test");

    let source = |service: &str| {
        format!(
            r#"pub boundary trait First {{ machine ping() reaches First; }}
pub boundary trait Second {{ machine ping() reaches Second; }}
pub machine dispatch()
reaches First + Second
invokes {service};
{{
    {service}::ping();
}}
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
    .expect("resolve invocation baseline");
    let baseline_reviews = compile_resolved_package_reviews(
        &baseline_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile invocation baseline");

    write_package(&live, &source("Second"));
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve invocation candidate");
    let candidate_reviews = compile_resolved_package_reviews(
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile invocation candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare invocation change");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::Callable)
        .expect("changed callable row");
    assert_eq!(conflict.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert_eq!(
        conflict.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render invocation conflict");
    assert!(rendered.contains("baseline_location synchronous_invocation package "));
    assert!(rendered.contains("candidate_location synchronous_invocation package "));

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn service_reach_changes_render_exact_authored_target_locations() {
    let live = temp_root("service-reach-location-live");
    let baseline_cache = temp_root("service-reach-location-baseline");
    let candidate_cache = temp_root("service-reach-location-candidate");
    let build_root = temp_root("service-reach-location-build");
    let context = ExternalSourceContext::derive(b"service-reach-location-conflict-test");

    let source = |service: &str| {
        format!(
            r#"pub boundary trait First {{ machine ping() reaches First; }}
pub boundary trait Second {{ machine ping() reaches Second; }}
pub machine dispatch()
reaches {service}
{{ }}
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
    .expect("resolve service-reach baseline");
    let baseline_reviews = compile_resolved_package_reviews(
        &baseline_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile service-reach baseline");

    write_package(&live, &source("Second"));
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve service-reach candidate");
    let candidate_reviews = compile_resolved_package_reviews(
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile service-reach candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare service-reach change");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::Callable)
        .expect("changed callable row");
    assert_eq!(conflict.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert_eq!(
        conflict.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render service-reach conflict");
    assert!(rendered.contains("baseline_location service_reach package "));
    assert!(rendered.contains("candidate_location service_reach package "));

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn operational_changes_render_exact_authored_clause_locations() {
    let live = temp_root("operational-location-live");
    let baseline_cache = temp_root("operational-location-baseline");
    let candidate_cache = temp_root("operational-location-candidate");
    let build_root = temp_root("operational-location-build");
    let context = ExternalSourceContext::derive(b"operational-location-conflict-test");

    let source = |clause: &str| format!("pub machine operate()\n{clause};\n{{ }}\n");
    write_package(&live, &source("suspends"));
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve operational baseline");
    let baseline_reviews = compile_resolved_package_reviews(
        &baseline_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile operational baseline");

    write_package(&live, &source("blocks"));
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve operational candidate");
    let candidate_reviews = compile_resolved_package_reviews(
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile operational candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare operational change");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::Callable)
        .expect("changed callable row");
    assert_eq!(conflict.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert_eq!(
        conflict.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render operational conflict");
    assert!(rendered.contains("baseline_location suspension package "));
    assert!(rendered.contains("candidate_location blocking package "));

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn external_executable_supply_changes_render_as_opaque_blocking_conflicts() {
    let live = temp_root("external-supply-live");
    let baseline_cache = temp_root("external-supply-baseline");
    let candidate_cache = temp_root("external-supply-candidate");
    let build_root = temp_root("external-supply-build");
    let context = ExternalSourceContext::derive(b"external-supply-conflict-test");

    let source = |symbol: &str| {
        format!(
            r#"pub boundary trait ForeignSurface {{
    machine invoke() reaches ForeignSurface;
}}
pub machine invoke_leaf()
    satisfies ForeignSurface::invoke
    via Binding::DllImport("omega-host", "{symbol}");
"#,
        )
    };
    write_package(&live, &source("invoke_v1"));
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve external-supply baseline");
    let baseline_reviews = compile_resolved_package_reviews(
        &baseline_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile external-supply baseline");

    let initial_conflicts = compare_review_only_initial_capabilities(
        &baseline_reviews,
        &baseline_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare external executable supply with empty admission baseline");
    let initial_external = initial_conflicts
        .packages()
        .iter()
        .find_map(|package| {
            package
                .conflicts()
                .iter()
                .find(|conflict| {
                    conflict.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply
                })
                .map(|conflict| (package, conflict))
        })
        .expect("fresh external executable supply is an exact policy conflict");
    assert_eq!(
        initial_external.0.baseline(),
        &ReviewOnlyCapabilityConflictBaseline::EmptyAdmission
    );
    assert!(initial_external.0.baseline_resolution().is_none());
    assert_eq!(
        initial_external.1.change(),
        ReviewOnlyCapabilityConflictChange::Added
    );
    assert_eq!(
        initial_external.1.risk(),
        PackageReviewCanonicalRowRisk::OpaqueBlocking
    );
    initial_external
        .0
        .root_policy_decision(
            initial_external.1,
            ReviewOnlyRootPolicyDisposition::RejectCandidateChange,
        )
        .expect("fresh external supply accepts an exact root-policy decision");
    let initial_render = initial_conflicts
        .render_bounded(1024 * 1024)
        .expect("render empty-admission external supply conflict");
    assert!(initial_render.contains("baseline empty_admission\n"));
    assert!(
        initial_render
            .contains("change added\nkind external_executable_supply\nrisk opaque_blocking\n")
    );
    let initial_triage = triage_initial_install(&baseline_reviews);
    assert_eq!(
        initial_triage.disposition(),
        PackageTriageDisposition::BlockedCapabilityChange
    );
    assert!(initial_triage.decisions().iter().any(|decision| {
        decision
            .reasons()
            .contains(&PackageTriageReason::ExternalExecutableSupplyRequiresResolution)
    }));

    write_package(&live, &source("invoke_v2"));
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve external-supply candidate");
    let candidate_reviews = compile_resolved_package_reviews(
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile external-supply candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare external executable supply");
    let external_conflicts = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .filter(|conflict| {
            conflict.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply
        })
        .collect::<Vec<_>>();
    let [conflict] = external_conflicts.as_slice() else {
        panic!("expected exactly one external executable-supply conflict")
    };
    assert_eq!(
        conflict.risk(),
        PackageReviewCanonicalRowRisk::OpaqueBlocking
    );
    assert_eq!(
        conflict.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    assert!(conflict.is_blocking());
    assert!(!conflicts.packages().iter().any(|package| {
        package
            .conflicts()
            .iter()
            .any(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::Callable)
    }));
    assert!(
        conflicts
            .render_bounded(1024 * 1024)
            .expect("render external executable-supply conflict")
            .contains("change changed\nkind external_executable_supply\nrisk opaque_blocking\n")
    );

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn transparent_proposition_changes_render_exact_formula_custody() {
    let live = temp_root("transparent-proposition-live");
    let baseline_cache = temp_root("transparent-proposition-baseline");
    let candidate_cache = temp_root("transparent-proposition-candidate");
    let build_root = temp_root("transparent-proposition-build");
    let context = ExternalSourceContext::derive(b"transparent-proposition-conflict-test");

    write_package(&live, "pub proposition ready() = true;\n");
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve transparent proposition baseline");
    let baseline_reviews = compile_resolved_package_reviews(
        &baseline_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile transparent proposition baseline");

    write_package(&live, "pub proposition ready() = false;\n");
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve transparent proposition candidate");
    let candidate_reviews = compile_resolved_package_reviews(
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile transparent proposition candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare transparent proposition compatibility");
    let [package] = conflicts.packages() else {
        panic!("one changed package")
    };
    let [conflict] = package.conflicts() else {
        panic!("one changed transparent proposition row")
    };
    assert_eq!(
        conflict.kind(),
        PackageReviewCanonicalRowKind::PublicProposition
    );
    for source in [conflict.baseline_source(), conflict.candidate_source()] {
        assert!(
            source
                .and_then(PackageReviewCanonicalRowSource::authored_locations)
                .unwrap()
                .iter()
                .any(|location| {
                    location.role() == PackageReviewSourceLocationRole::PropositionFormula
                        && location.relative_path() == "main.omg"
                })
        );
    }
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render transparent proposition conflict");
    assert!(rendered.starts_with("OMEGA_PACKAGE_CAPABILITY_CONFLICTS_V20\n"));
    assert!(rendered.contains("baseline_location proposition_formula package "));
    assert!(rendered.contains("candidate_location proposition_formula package "));

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}

#[test]
fn public_domain_changes_render_exact_proof_fact_custody() {
    let live = temp_root("public-domain-proof-fact-live");
    let baseline_cache = temp_root("public-domain-proof-fact-baseline");
    let candidate_cache = temp_root("public-domain-proof-fact-candidate");
    let build_root = temp_root("public-domain-proof-fact-build");
    let context = ExternalSourceContext::derive(b"public-domain-proof-fact-conflict-test");

    write_package(
        &live,
        "pub data Packet { value: u32; }\npub domain Packet::Ready\nrequires self.value == 0;\n",
    );
    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public domain baseline");
    let baseline_reviews = compile_resolved_package_reviews(
        &baseline_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile public domain baseline");

    write_package(
        &live,
        "pub data Packet { value: u32; }\npub domain Packet::Ready\nrequires self.value == 1;\n",
    );
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve public domain candidate");
    let candidate_reviews = compile_resolved_package_reviews(
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile public domain candidate");

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare public domain proof facts");
    let conflict = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::PublicDomain)
        .expect("changed public domain row");
    for source in [conflict.baseline_source(), conflict.candidate_source()] {
        assert!(
            source
                .and_then(PackageReviewCanonicalRowSource::authored_locations)
                .expect("public domain source locations")
                .iter()
                .any(|location| {
                    location.role() == PackageReviewSourceLocationRole::ProofFact
                        && location.relative_path() == "main.omg"
                })
        );
    }
    let rendered = conflicts
        .render_bounded(1024 * 1024)
        .expect("render public domain proof-fact conflict");
    assert!(rendered.starts_with("OMEGA_PACKAGE_CAPABILITY_CONFLICTS_V20\n"));
    assert!(rendered.contains("baseline_location proof_fact package "));
    assert!(rendered.contains("candidate_location proof_fact package "));

    let _ = std::fs::remove_dir_all(live);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(build_root);
}
