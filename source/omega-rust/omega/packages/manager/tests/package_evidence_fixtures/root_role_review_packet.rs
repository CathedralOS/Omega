use super::*;

fn resolve_role_probe(
    live_root: &Path,
    context: ExternalSourceContext,
    cache: &Path,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    let storage = SourceResolverStorage::for_hardened_base(cache).map_err(|error| {
        ResolveExternalLocalPackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    resolve_external_local_project_closure_with_storage(
        live_root,
        context,
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
}

fn write_role_probe(live_root: &Path, role: &str) {
    std::fs::create_dir_all(live_root).expect("create root-role review fixture");
    std::fs::write(
        live_root.join("build.omg"),
        format!(
            "target windows_x86_64 {{ }}\n\nmachine build(builder: &mut Build) {{\n    builder.{role}(\"root-role-review-probe\");\n}}\n"
        ),
    )
    .expect("write root-role build declaration");
    std::fs::write(
        live_root.join("main.omg"),
        "pub machine add(left: u64, right: u64) -> u64 { left + right }\n",
    )
    .expect("write root-role package source");
}

#[test]
fn recovered_baseline_source_review_preserves_directional_root_role_blockers() {
    let live = temp_root("root-role-live");
    let package_cache = temp_root("root-role-package-cache");
    let application_cache = temp_root("root-role-application-cache");
    let build_root = temp_root("root-role-build");
    let context = ExternalSourceContext::derive(b"root-role-review-packet");

    write_role_probe(&live, "package");
    let package_sources = resolve_role_probe(&live, context.clone(), &package_cache)
        .expect("resolve package-role source closure");
    let package_reviews = compile_resolved_package_candidate_reviews(
        &package_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile package-role reviews");
    let package_baseline = ReviewOnlyBaselineCapsule::capture(
        &package_sources,
        &package_reviews,
        ReviewOnlyBaselineLimits::default(),
    )
    .expect("capture package-role baseline");

    write_role_probe(&live, "application");
    let application_sources = resolve_role_probe(&live, context, &application_cache)
        .expect("resolve application-role source closure");
    let application_reviews = compile_resolved_package_candidate_reviews(
        &application_sources.for_exact_target(omega_target::TargetProfile::WindowsX64),
        &build_root,
    )
    .expect("compile application-role reviews");
    let application_baseline = ReviewOnlyBaselineCapsule::capture(
        &application_sources,
        &application_reviews,
        ReviewOnlyBaselineLimits::default(),
    )
    .expect("capture application-role baseline");

    for (baseline, baseline_custody, candidate_reviews, candidate_sources, reason) in [
        (
            &package_baseline,
            package_sources.custodies(),
            &application_reviews,
            &application_sources,
            PackageTriageReason::RootLostDependencyCompatibility,
        ),
        (
            &application_baseline,
            application_sources.custodies(),
            &package_reviews,
            &package_sources,
            PackageTriageReason::RootLostApplicationActivation,
        ),
    ] {
        let standalone = triage_review_update_from_baseline(
            baseline,
            candidate_reviews,
            candidate_sources,
            &BTreeSet::new(),
        );
        let packet = assemble_update_source_review_from_baseline(
            baseline,
            candidate_reviews,
            baseline_custody,
            candidate_sources,
            omega_package_manager::review::PackageSourceReviewLimits::default(),
        )
        .expect("assemble root-role-aware source-review packet");
        assert_eq!(packet.triage(), &standalone);
        let root = standalone
            .decisions()
            .iter()
            .find(|decision| decision.candidate_key() == Some(candidate_sources.graph().root()))
            .expect("root-role triage retains candidate root");
        assert_eq!(
            root.disposition(),
            PackageTriageDisposition::BlockedCapabilityChange
        );
        assert!(root.reasons().contains(&reason));
        let rendered = packet
            .render_bounded(1024 * 1024)
            .expect("render root-role-aware source-review packet");
        assert!(rendered.contains(&format!(
            "reason {}\n",
            match reason {
                PackageTriageReason::RootLostDependencyCompatibility => {
                    "root_lost_dependency_compatibility"
                }
                PackageTriageReason::RootLostApplicationActivation => {
                    "root_lost_application_activation"
                }
                _ => unreachable!("loop contains only directional root-role reasons"),
            }
        )));
    }

    for path in [live, package_cache, application_cache, build_root] {
        let _ = std::fs::remove_dir_all(path);
    }
}
