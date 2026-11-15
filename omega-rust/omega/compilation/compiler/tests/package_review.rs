//! Review publishes source-free evidence under the caller's exact byte budget.

#[path = "support/build_sequence.rs"]
mod build_sequence;

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[path = "support/native_entry_controls.rs"]
mod native_entry_controls;

use compiler::{CheckedCompileRequest, CompilationSession, PackageReviewFailure};
use review_evidence::PackageReviewProjectionError;
use std::path::{Path, PathBuf};

fn kept_fixture(relative: &str) -> PathBuf {
    fixture_package_inputs::repo_root()
        .join("tests/fixtures")
        .join(relative)
}

#[test]
fn reusable_package_checks_declarations_without_deploying_unused_float_bodies() {
    // These libraries have no program entry. Their UEFI target request belongs
    // to this package check; the positive also covers ordinary source checking.
    let target = "uefi_x86_64";
    let mut failures = Vec::new();
    for (fixture, refusal) in [
        ("pass/float/rootless_target_realization_scope", None),
        (
            "packages/selected-provider-unsupported-target",
            Some("the target declares no floating-point realization table"),
        ),
        (
            "fail/constants/unused_scalar_constant_range",
            Some("scalar constant `VALUE` is invalid"),
        ),
        (
            "fail/proofs/ordinary_denotational_call_requires_normal_return",
            Some("has no observation-free, reach-free, crash-free value denotation"),
        ),
    ] {
        let source = kept_fixture(fixture).join("main.omg");
        let result = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
            &source,
            Some(target),
        )
        .and_then(|mut prepared| {
            fixture_package_inputs::check_fixture(&mut prepared, target, Default::default())
        });
        match (result, refusal) {
            (Ok(_), None) => {}
            (Err(diagnostics), Some(fragment))
                if diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(fragment)) => {}
            (Err(diagnostics), Some(fragment)) => failures.push(format!(
                "{fixture}: expected {fragment:?}, got {diagnostics:?}"
            )),
            (Ok(_), Some(fragment)) => {
                failures.push(format!("{fixture}: accepted instead of {fragment:?}"));
            }
            (Err(diagnostics), None) => failures.push(format!("{fixture}: {diagnostics:?}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn execute_build(
    session: &mut CompilationSession,
    request: CheckedCompileRequest,
) -> compiler::PackageBuild {
    let mut invocation = session
        .begin_package_build(request)
        .expect("admit package Build");
    let returned = build_sequence::return_package(&mut invocation);
    session
        .complete_package_build(invocation, returned)
        .expect("complete package Build")
}

#[test]
fn review_retains_installation_reach_across_staggered_dependency_resolution() {
    let target = "macos_arm64";
    let fixture = kept_fixture("pass/providers/staggered_installation_reach_exit/main.omg");
    let prepared =
        fixture_package_inputs::reviewed_repository_fixture_package_inputs(&fixture, Some(target))
            .expect("installation reach fixture inputs");
    let mut session = CompilationSession::default();
    let build = execute_build(
        &mut session,
        CheckedCompileRequest {
            package_inputs: prepared.package_inputs.clone(),
            ..CheckedCompileRequest::new(&prepared.root_path, Some(target))
        },
    );
    let checked = session
        .check_package_build(build)
        .expect("checked installation closure");
    let module = checked.terminal_publication().accepted().unwrap().module();
    let [dependency] = module
        .root_service_reach
        .installation_dependencies
        .as_slice()
    else {
        panic!("the root reaches one installation-bound requirement");
    };
    let requirement = module
        .identities
        .text(dependency.requirement_identity)
        .unwrap();
    assert!(requirement.contains("A::visit"), "{requirement}");
    assert_eq!(dependency.upper_bound.len(), 2);
    let abstract_requirements = module.installation_reach_requirements();
    assert_eq!(abstract_requirements.len(), 4);
    for boundary in &module.boundary_machines {
        if boundary.service_reach_is_installation_bound {
            assert!(boundary.fixed_service_reach.is_empty());
            assert_eq!(boundary.published_service_ceiling.len(), 2);
        }
    }
    let encoded = terminal_codec::encode_module(module).expect("retained installation module");
    let decoded =
        terminal_codec::decode_module(&encoded).expect("installation declaration roundtrip");
    assert_eq!(
        decoded.installation_reach_requirements(),
        abstract_requirements
    );
    let mut fixed = decoded.clone();
    let required = fixed
        .boundary_machines
        .iter_mut()
        .find(|boundary| boundary.identity == dependency.requirement_identity)
        .expect("exact root requirement");
    required.service_reach_is_installation_bound = false;
    assert!(matches!(
        terminal_codec::decode_module(&terminal_codec::encode_module(&fixed).unwrap()),
        Err(terminal_codec::CodecError::MalformedStructuralFoundation(
            "installation reach dependency names a fixed requirement"
        ))
    ));
    let selected = checked.execution().plans();
    assert_eq!(
        selected.resolve_root_service_reach(module).unwrap().len(),
        2
    );
    let reach = |provider: &str| {
        let plan = selected
            .plans()
            .iter()
            .find(|plan| plan.provider_type == provider)
            .expect("selected named provider");
        &selected
            .installation_reach_resolutions()
            .iter()
            .find(|row| row.provider_plan_report_identity == plan.report_fingerprint())
            .expect("selected installation row")
            .resolved_row
    };
    let first = reach("BProvider");
    let last = reach("DProvider");
    assert_eq!(first.len(), 1);
    assert_eq!(last.len(), 1);
    assert_ne!(
        first, last,
        "different children must expose a lost contribution"
    );
    let mut expected = first.clone();
    expected.extend(last.iter().cloned());
    expected.sort();
    expected.dedup();
    assert_eq!(reach("CProvider"), last);
    assert_eq!(
        reach("AProvider"),
        &expected,
        "a child resolved in an earlier round remains in the union"
    );
    let build = execute_build(
        &mut session,
        CheckedCompileRequest {
            package_inputs: prepared.package_inputs,
            ..CheckedCompileRequest::new(&prepared.root_path, Some(target))
        },
    );
    let reviewed = session
        .review_package_build(build, usize::MAX)
        .expect("exact installed reach remains reviewable");
    let bytes = reviewed.evidence.policy.canonical_bytes().unwrap();
    let recovered = review_evidence::record::PackagePolicyBaseline::recover_canonical(
        &bytes,
        review_evidence::encoding::PackagePolicyRecoveryLimits::default(),
    )
    .unwrap();
    for (provider, resolved) in [
        ("AProvider", &["X", "Y"][..]),
        ("BProvider", &["X"][..]),
        ("CProvider", &["Y"][..]),
        ("DProvider", &["Y"][..]),
    ] {
        let plan = recovered
            .selected_providers()
            .plans()
            .iter()
            .find(|plan| plan.provider_type() == provider)
            .unwrap();
        let [row] = plan.rows() else {
            panic!("one exact visited requirement for {provider}");
        };
        let reach = row
            .installation_reach()
            .expect("saved selected installation reach");
        let names = |row: &[review_evidence::record::PackageReviewNominalIdentity]| {
            row.iter()
                .map(|name| name.path().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(reach.upper_bound()), ["X", "Y"]);
        assert_eq!(names(reach.resolved()), resolved);
    }
}

#[test]
fn narrow_installation_bound_uses_the_selected_child_row() {
    let target = "linux_x86_64";
    let mut failures = Vec::new();
    for (fixture, refusal) in [
        ("pass/providers/narrow_installation_reach", None),
        (
            "fail/providers/narrow_installation_reach",
            Some("exceeds its published upper bound"),
        ),
    ] {
        let source = kept_fixture(fixture).join("main.omg");
        let result = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
            &source,
            Some(target),
        )
        .and_then(|mut prepared| {
            fixture_package_inputs::check_fixture(&mut prepared, target, Default::default())
        });
        match (result, refusal) {
            (Ok(checked), None) => {
                let selected = checked.execution().plans();
                let plan = selected
                    .plans()
                    .iter()
                    .find(|plan| plan.provider_type == "AProvider")
                    .expect("selected bounded parent");
                let row = selected
                    .installation_reach_resolutions()
                    .iter()
                    .find(|row| row.provider_plan_report_identity == plan.report_fingerprint())
                    .expect("resolved parent row");
                assert_eq!(row.resolved_row.len(), 1);
                let module = checked.terminal_publication().accepted().unwrap().module();
                assert_eq!(selected.resolve_root_service_reach(module).unwrap().len(), 2);
            }
            (Err(errors), Some(expected))
                if errors.iter().any(|error| error.message.contains(expected)) => {}
            (Err(errors), _) => failures.push(format!("{fixture}: {errors:?}")),
            (Ok(_), Some(expected)) => {
                failures.push(format!("{fixture}: accepted instead of {expected:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn review_product_keeps_exact_custody_and_policy_budget() {
    let target = "linux_x86_64";
    let fixture = kept_fixture("pass/calls/machine_self_call_recursion_compile/main.omg");
    let prepared =
        fixture_package_inputs::reviewed_repository_fixture_package_inputs(&fixture, Some(target))
            .expect("fixture has admitted package inputs");
    let inputs = prepared.package_inputs.expect("package-aware fixture");
    let package = inputs.root();
    let mut session = CompilationSession::default();
    let mut review = |maximum_bytes| {
        let build = execute_build(
            &mut session,
            CheckedCompileRequest {
                package_inputs: Some(inputs.clone()),
                ..CheckedCompileRequest::new(&prepared.root_path, Some(target))
            },
        );
        session.review_package_build(build, maximum_bytes)
    };
    let first = review(usize::MAX).expect("review product");
    let bytes = first.evidence.policy_canonical_bytes;
    assert_eq!(first.evidence.policy.package(), package);
    assert_eq!(first.evidence.build_handoff.package(), package);
    assert_eq!(
        first.evidence.source_consumption_commitment,
        first.evidence.build_handoff.source_consumption_commitment()
    );
    assert_eq!(
        first
            .evidence
            .policy
            .canonical_bytes()
            .expect("policy encoding")
            .len(),
        bytes
    );
    assert!(bytes > 0);
    assert!(matches!(
        review(bytes - 1),
        Err(PackageReviewFailure::Projection {
            failure: PackageReviewProjectionError::PolicyBudget,
            ..
        })
    ));
    let exact = review(bytes).expect("exact budget is sufficient");
    assert_eq!(exact.evidence.policy, first.evidence.policy);
    assert_eq!(
        exact.evidence.source_consumption_commitment,
        first.evidence.source_consumption_commitment
    );
}

#[test]
fn review_policy_round_trip_preserves_inferred_imported_reach() {
    // Review the package that declares the public helper and its private call.
    let fixture = kept_fixture("pass/effects/inferred_imported_reach/support/main.omg");
    let recovered = reviewed_and_recovered(&fixture, "macos_arm64");
    let callable = recovered
        .callables()
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("fetch_mark"))
        .expect("exported inferred helper");
    assert!(callable.declared_service_reach().is_none());
    let realized = callable
        .checked_service_reach()
        .realized()
        .expect("checked body");
    assert_eq!(realized.len(), 1);
    assert_eq!(realized[0].path(), "Mark");
    assert_eq!(callable.published_service_reach(), Some(realized));

    let mark = recovered
        .public_traits()
        .iter()
        .find(|declaration| declaration.identity() == &realized[0])
        .expect("the support package's exact Mark declaration");
    let [read] = mark.requirements() else {
        panic!("one Mark requirement");
    };
    let consumer = reviewed_and_recovered(
        &kept_fixture("pass/effects/inferred_imported_reach/consumer/main.omg"),
        "macos_arm64",
    );
    let [plan] = consumer.selected_providers().plans() else {
        panic!("one selected MarkerMark plan");
    };
    assert_eq!(plan.target(), "macos_arm64");
    assert_eq!(plan.schema_declaration(), mark.identity());
    let [row] = plan.rows() else {
        panic!("one checked MarkerMark implementation");
    };
    let [method] = plan.methods() else {
        panic!("one imported requirement contract");
    };
    assert_eq!(row.requirement(), read.identity());
    assert_eq!(row.requirement(), method.requirement());
    assert_eq!(method.requirement_owner(), mark.identity());

    let owner = review_evidence::record::PackageReviewNominalOwner::Package(consumer.package());
    assert_ne!(owner, mark.identity().owner());
    let provider = plan
        .provider_type_declaration()
        .expect("the local provider");
    assert_eq!(provider.path(), "MarkerMark");
    assert_eq!(provider.owner(), owner);
    let realization = row.realization().expect("the checked local body");
    assert_eq!(realization.path(), "MarkerMark::read");
    assert_eq!(realization.owner(), owner);
    assert!(
        consumer
            .public_traits()
            .iter()
            .all(|declaration| declaration.identity().path() != "Mark")
    );
    assert!(
        consumer
            .callables()
            .callables()
            .iter()
            .all(|callable| callable.identity().path() != "Mark::read")
    );
}

#[test]
fn build_service_audit_rejects_an_ordinary_trait() {
    let fixture = kept_fixture("fail/build/service_audit_requires_boundary/main.omg");
    let expected = std::fs::read_to_string(fixture.with_file_name("expected.txt")).unwrap();
    let Err(diagnostics) =
        fixture_package_inputs::reviewed_repository_fixture_package_inputs(&fixture, None)
    else {
        panic!("an ordinary trait cannot register a service audit");
    };
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("one refusal at the authored operand: {diagnostics:?}");
    };
    assert_eq!(diagnostic.message, expected.trim_end());
    let location = diagnostic
        .resolved_location
        .as_ref()
        .expect("the authored Build operand location");
    assert_eq!(location.path.file_name().unwrap(), "build.omg");
    assert_eq!((location.line, location.column), (3, 12));
}

#[test]
fn review_policy_round_trip_keeps_unsupplied_driver_requirements() {
    let recovered = reviewed_and_recovered(
        &kept_fixture("pass/effects/root_service_audit/driver/main.omg"),
        "macos_arm64",
    );
    assert!(recovered.selected_providers().plans().is_empty());

    let traits = recovered.public_traits();
    let driver = traits
        .iter()
        .find(|shape| shape.identity().path() == "Driver")
        .expect("public Driver interface");
    let [requirement] = driver.requirements() else {
        panic!("one Driver requirement");
    };
    assert_eq!(requirement.identity().path(), "Driver::drive");
    let callable = recovered
        .callables()
        .callables()
        .iter()
        .find(|callable| callable.identity() == requirement.identity())
        .expect("the exact bodyless Driver requirement");
    assert_eq!(format!("{:?}", callable.supply()), "Boundary");
    assert!(callable.checked_service_reach().realized().is_none());

    let expected = ["DependencyOnly", "MotorControl", "Untaken"].map(|name| {
        traits
            .iter()
            .find(|shape| shape.identity().path() == name)
            .expect("authored service")
            .identity()
            .clone()
    });
    assert_eq!(callable.declared_service_reach(), Some(expected.as_slice()));
    assert_eq!(callable.service_reach_dependency().concrete(), expected);
}

#[test]
fn review_policy_round_trip_preserves_authored_reach_slack() {
    let recovered = reviewed_and_recovered(
        &kept_fixture("pass/effects/optional_reach_declared_slack_exit/main.omg"),
        "macos_arm64",
    );
    let callable = recovered
        .callables()
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("read_mark"))
        .expect("exported bounded helper");
    let declared = callable
        .declared_service_reach()
        .expect("authored wider ceiling");
    let realized = callable
        .checked_service_reach()
        .realized()
        .expect("checked body");
    assert_eq!(declared.len(), 2);
    assert_eq!(realized.len(), 1);
    assert_eq!(realized[0].path(), "Mark");
    let requirement = recovered
        .callables()
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "Mark::read")
        .expect("the exact boundary requirement");
    assert_eq!(requirement.declared_service_reach(), Some(realized));
    assert_eq!(requirement.service_reach_dependency().concrete(), realized);
    let [audited] = recovered.service_audits() else {
        panic!("one executed service audit");
    };
    assert_eq!(audited.path(), "Unused");
    assert!(declared.contains(audited));
    assert_eq!(callable.published_service_reach(), Some(declared));
    assert!(recovered.slack_uses().iter().any(|row| {
        row.callable() == callable.identity()
            && row.service() == audited
            && row.class()
                == review_evidence::record::PackageReviewDangerousAuthorityClass::ConsumerAudit
    }));
    let service = recovered
        .public_traits()
        .iter()
        .find(|declaration| declaration.identity() == audited)
        .expect("the exact authored service remains a public trait");
    assert!(service.requirements().is_empty());
}

#[test]
fn review_policy_round_trip_distinguishes_explicit_and_inferred_empty_reach() {
    let recovered = reviewed_and_recovered(
        &kept_fixture("pass/effects/optional_reach_empty_exit/main.omg"),
        "macos_arm64",
    );
    let callables = recovered.callables().callables();
    let explicit = callables
        .iter()
        .find(|callable| callable.identity().path().contains("read_mark"))
        .expect("explicit empty helper");
    let inferred = callables
        .iter()
        .find(|callable| callable.identity().path().contains("inferred_empty"))
        .expect("inferred empty helper");
    assert_eq!(explicit.declared_service_reach(), Some([].as_slice()));
    assert!(inferred.declared_service_reach().is_none());
    assert_eq!(explicit.published_service_reach(), Some([].as_slice()));
    assert_eq!(inferred.published_service_reach(), Some([].as_slice()));
}

fn historical_constant_source() -> PathBuf {
    fixture_package_inputs::repo_root()
        .join("samples/cli/facts/target_layout_derived_constant/main.omg")
}

fn historical_constant_snapshot() -> package_compilation::BuildSnapshotRequest {
    use package_compilation::{BuildSourceCaptureObligation, BuildSourceCaptureRequest};
    // Each nested placement is a different project with its own Answer.
    let source = BuildSourceCaptureRequest::new(["build.omg", "main.omg"].map(|file| {
        (
            file.as_bytes().to_vec(),
            BuildSourceCaptureObligation::Required,
        )
    }))
    .expect("exact historical source inventory");
    package_compilation::BuildSnapshotRequest::scoped([], source)
}

fn historical_answer_workspace(source: &Path, directory: &Path) -> PathBuf {
    std::fs::create_dir_all(directory).unwrap();
    std::fs::copy(source, directory.join("main.omg")).unwrap();
    std::fs::copy(
        source.with_file_name("build.omg"),
        directory.join("build.omg"),
    )
    .unwrap();
    directory.join("main.omg")
}

fn select_historical_linux_answer(project: &Path, original_build: &str, provider: Option<&str>) {
    let original = "Build::select_provider<Answer, Small>(builder, linux_x86_64);";
    assert_eq!(original_build.matches(original).count(), 1);
    let statement = provider.map_or_else(String::new, |provider| {
        format!("Build::select_provider<Answer, {provider}>(builder, linux_x86_64);")
    });
    std::fs::write(
        project.with_file_name("build.omg"),
        original_build.replacen(original, &statement, 1),
    )
    .unwrap();
}

#[test]
fn historical_constant_build_retains_its_authored_answer_selections() {
    use package_compilation::provider_selection_codec::{
        decode_package_provider_selections, encode_package_provider_selections,
    };

    fn authored_name(declaration: &symbols::RetainedDeclaration) -> &str {
        let span = declaration.span();
        &declaration.source().source[span.start..span.end]
    }

    fn check(
        source: &Path,
        session_path: &Path,
        linux_provider: Option<&str>,
    ) -> semantic_vocabulary::PackageKeyIdentity {
        let target = target::TargetProfile::LinuxX64;
        let (_session, build, _) = captured_package_build(
            source,
            session_path,
            target,
            Some(historical_constant_snapshot()),
        );
        let handoff = build.build_handoff();
        assert_eq!(handoff.target(), target);
        let bytes = encode_package_provider_selections(
            handoff.provider_selections(),
            handoff.selection_observation(),
        );
        let (selections, observation) =
            decode_package_provider_selections(&bytes).expect("recover retained Build selections");
        assert!(observation.is_some());
        assert_eq!(
            encode_package_provider_selections(&selections, observation),
            bytes
        );
        let mut answers = selections
            .iter()
            .filter(|selection| authored_name(&selection.subject) == "Answer")
            .map(|selection| {
                for declaration in [&selection.subject, &selection.provider] {
                    let source = declaration.source();
                    assert_eq!(source.package_identity, Some(handoff.package()));
                    assert_eq!(
                        source.path.strip_prefix(&source.package_root).unwrap(),
                        Path::new("main.omg")
                    );
                }
                (
                    selection.target.canonical_name(),
                    authored_name(&selection.provider),
                )
            })
            .collect::<Vec<_>>();
        answers.sort_unstable();
        let mut expected = vec![
            ("linux_arm64", "Small"),
            ("macos_arm64", "Large"),
            ("windows_x86_64", "Large"),
        ];
        if let Some(provider) = linux_provider {
            expected.insert(1, ("linux_x86_64", provider));
        }
        assert_eq!(answers, expected);
        handoff.package()
    }

    let source = historical_constant_source();
    let workspace = platform_custody::temporary_directory::TemporaryDirectory::create(
        "historical-answer-build",
        false,
    )
    .unwrap();
    check(&source, &workspace.path().join("original"), Some("Small"));

    let project = historical_answer_workspace(&source, &workspace.path().join("project"));
    let original_build = std::fs::read_to_string(source.with_file_name("build.omg")).unwrap();
    let package = check(&project, &workspace.path().join("small"), Some("Small"));
    select_historical_linux_answer(&project, &original_build, Some("Same"));
    assert_eq!(
        check(&project, &workspace.path().join("same"), Some("Same")),
        package
    );
    select_historical_linux_answer(&project, &original_build, None);
    assert_eq!(
        check(&project, &workspace.path().join("absent"), None),
        package
    );
}

#[test]
fn historical_constant_closure_requires_exact_target_and_provider() {
    use orchestration::{BuildRequest, BuildTargets, NativeDestination, ProjectProduct};

    fn produce(root: &Path, target_name: &str, product: ProjectProduct) -> compiler::CompileReport {
        let profile = target::TargetProfile::from_canonical_target_name(target_name).unwrap();
        let outcomes = orchestration::build(BuildRequest {
            stage_validators: Default::default(),
            root_path: root.to_path_buf(),
            build_directory: Some(std::env::temp_dir().join(format!(
                "omega-layout-closure-{}-{target_name}",
                std::process::id(),
            ))),
            targets: BuildTargets::Single(profile),
            product,
            timings: false,
            offline: true,
            verify: false,
            require_package_project: true,
            optimization_rollback: Default::default(),
            build_snapshot: Some(historical_constant_snapshot()),
        })
        .expect("admit the source-backed package request");
        let [outcome] = <[_; 1]>::try_from(outcomes)
            .unwrap_or_else(|_| panic!("one requested historical layout target"));
        assert_eq!(outcome.target, profile);
        outcome
            .result
            .unwrap_or_else(|error| panic!("source reaches its requested product: {error}"))
            .into_report()
            .expect("report of the requested product")
            .report
    }

    let root = historical_constant_source();
    let workspace = platform_custody::temporary_directory::TemporaryDirectory::create(
        "historical-answer-closure",
        false,
    )
    .unwrap();
    // Compare two actual selections for one exact package. A second
    // package's identity could independently falsify the substitution.
    let project = historical_answer_workspace(&root, &workspace.path().join("project"));
    let retained = produce(&project, "linux_x86_64", ProjectProduct::Terminal)
        .into_retained_terminal_artifact()
        .expect("native proposal");
    let proposal = retained
        .native_realization_proposal()
        .expect("native proposal");
    let decoded =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&retained.artifact().to_bytes())
            .expect("canonical Terminal roundtrip");
    assert_eq!(
        decoded.manifest().semantic(),
        retained.artifact().manifest().semantic()
    );
    let view = decoded.view();
    let plans = proposal.providers().selection.plans();
    let preimages =
        backend_plan::matching_folded_constant_dependencies(view, "linux_x86_64", plans.iter())
            .expect("native admission rejoins exact real plans");
    assert!(
        !preimages.is_empty(),
        "folding must retain the selected extent provider"
    );
    let admit = |target, inputs: &[Vec<u8>]| {
        terminal_installation::admit_provider_installation(view, &[], &[])
            .expect("historical-only provider catalog")
            .admit_folded_constant_closure(view, target, inputs)
    };
    admit("linux_x86_64", &preimages).expect("exact historical closure");
    assert_eq!(
        admit("linux_arm64", &preimages).unwrap_err(),
        "folded constant closure belongs to another target"
    );
    assert_eq!(
        admit("linux_x86_64", &[]).unwrap_err(),
        "folded constant selected realization closure is incomplete or repeated"
    );
    let mut repeated = preimages.clone();
    repeated.push(preimages[0].clone());
    assert_eq!(
        admit("linux_x86_64", &repeated).unwrap_err(),
        "folded constant selected realization closure is incomplete or repeated"
    );
    let digests = view
        .module()
        .folded_constants
        .dependencies
        .iter()
        .filter(|dependency| dependency.target == "linux_x86_64")
        .map(|dependency| dependency.plan.to_vec())
        .collect::<Vec<_>>();
    assert_eq!(
        admit("linux_x86_64", &digests).unwrap_err(),
        "folded constant normalized plan input differs from its retained commitment"
    );

    let original_build = std::fs::read_to_string(root.with_file_name("build.omg"))
        .expect("authored Build selections");
    select_historical_linux_answer(&project, &original_build, Some("Same"));
    let alternate = produce(&project, "linux_x86_64", ProjectProduct::Terminal)
        .into_retained_terminal_artifact()
        .expect("alternate native proposal");
    let alternate_plans = alternate
        .native_realization_proposal()
        .unwrap()
        .providers()
        .selection
        .plans();
    let reason = backend_plan::matching_folded_constant_dependencies(
        view,
        "linux_x86_64",
        alternate_plans.iter(),
    )
    .unwrap_err();
    assert!(
        reason.contains("differs from the selected provider plan"),
        "{reason}"
    );
    let alternate_inputs = backend_plan::matching_folded_constant_dependencies(
        alternate.artifact().view(),
        "linux_x86_64",
        alternate_plans.iter(),
    )
    .expect("alternate provider's own real closure");
    assert_eq!(
        admit("linux_x86_64", &alternate_inputs).unwrap_err(),
        "folded constant normalized plan input differs from its retained commitment"
    );
    // This emits a Windows product but executes only its Terminal semantics on
    // this host. The frozen source's branch traps unless the derived value is
    // exactly 9 (the Windows provider extent 8 plus 1), rather than Linux's 5.
    let windows = produce(
        &root,
        "windows_x86_64",
        ProjectProduct::Native {
            destination: NativeDestination::Retained,
            private_entry_ram: Vec::new(),
            installation_assumptions: Vec::new(),
        },
    );
    let artifact = windows
        .retained_native_artifact()
        .expect("Windows native emission");
    let observation = terminal_runtime::execute(terminal_runtime::TerminalRuntimeRequest {
        native_image: None,
        entry: None,
        filesystem: Default::default(),
        native_services: Default::default(),
        native_provider: None,
        x86_flags: None,
        memory_ordering: None,
        provider_proposal: None,
        artifact: artifact.psi_artifact(),
        target: None,
        provisions: windows.runtime_provisions(),
        work_budget: 100_000,
        domain_exit: terminal_runtime::TerminalDomainExitDelegation::IsolatedContainingDomain,
        standard_input: None,
        clock: None,
    })
    .expect("Windows Terminal execution on this host");
    let complete = match &observation.status {
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit,
        ) => true,
        terminal_interpreter::TerminalExecutionStatus::ExternalCompletion(completion) => {
            completion.return_code == 0
        }
        _ => false,
    };
    assert!(complete, "{:?}", observation.status);
    println!("windows_terminal_exit=0 windows_derived_value=9 foreign_native_executed=false");

    println!(
        "canonical_roundtrip=ok exact_plan=admitted target_mismatch=rejected absent_plan=rejected repeated_plan=rejected digest_substitution=rejected equal_provider_substitution=rejected"
    );
}

#[test]
fn review_records_dynamic_slot_origin_and_qualification() {
    let target = "linux_x86_64";
    let fixture = kept_fixture("pass/traits/runtime_dyn_slot_metadata_exit/main.omg");
    let prepared =
        fixture_package_inputs::reviewed_repository_fixture_package_inputs(&fixture, Some(target))
            .expect("slot metadata fixture inputs");
    let mut session = CompilationSession::default();
    let build = execute_build(
        &mut session,
        CheckedCompileRequest {
            package_inputs: prepared.package_inputs,
            ..CheckedCompileRequest::new(&prepared.root_path, Some(target))
        },
    );
    let reviewed = session
        .review_package_build(build, usize::MAX)
        .expect("public slot metadata review");
    let window = reviewed
        .evidence
        .policy
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path().ends_with("Window"))
        .expect("public Window trait");
    let view = window
        .requirements()
        .iter()
        .find(|requirement| requirement.identity().path().contains("view"))
        .unwrap_or_else(|| {
            panic!(
                "view requirement missing from {}",
                window
                    .requirements()
                    .iter()
                    .map(|requirement| requirement.identity().path())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        });
    let origins = view.returned_borrow_origins();
    assert_eq!(origins.len(), 1, "returned-borrow origin roster");
    assert_eq!(origins[0].parameter_index(), Some(0));
    assert!(origins[0].invocation_carrier().is_none());
    let qualifications = view.parameter_qualification_preconditions();
    assert_eq!(qualifications.len(), 1, "qualification precondition roster");
    assert_eq!(qualifications[0].structural_index(), 0);
    assert!(
        qualifications[0].domain().path().contains("Utf8"),
        "{}",
        qualifications[0].domain().path()
    );
    let bytes = reviewed
        .evidence
        .policy
        .canonical_bytes()
        .expect("encode slot metadata policy");
    let recovered = review_evidence::record::PackagePolicyBaseline::recover_canonical(
        &bytes,
        review_evidence::encoding::PackagePolicyRecoveryLimits::default(),
    )
    .expect("recover slot metadata policy");
    let recovered_view = recovered
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path().ends_with("Window"))
        .expect("recovered Window")
        .requirements()
        .iter()
        .find(|requirement| requirement.identity().path().contains("view"))
        .expect("recovered view");
    assert_eq!(
        recovered_view.returned_borrow_origins(),
        view.returned_borrow_origins()
    );
    assert_eq!(
        recovered_view.parameter_qualification_preconditions(),
        view.parameter_qualification_preconditions()
    );
}

/// Review one repository fixture and strictly recover the policy it published.
/// Capture and recovery are separate owners of the same binder and identity
/// vocabulary; only a fixture that crosses both can notice them disagreeing.
fn reviewed_and_recovered(
    fixture: &Path,
    target: &str,
) -> review_evidence::record::PackagePolicyBaseline {
    use review_evidence::encoding::PackagePolicyRecoveryLimits;
    use review_evidence::record::PackagePolicyBaseline;

    let workspace = platform_custody::temporary_directory::TemporaryDirectory::create(
        "repository-package-review",
        false,
    )
    .unwrap();
    let target = target::TargetProfile::from_omega_target_name(Some(target)).unwrap();
    let (mut session, build, _) = captured_package_build(fixture, workspace.path(), target, None);
    let reviewed = session
        .compiler()
        .review_package_build(build, usize::MAX)
        .expect("fixture review");
    let bytes = reviewed
        .evidence
        .policy
        .canonical_bytes()
        .expect("encode policy");
    let recovered =
        PackagePolicyBaseline::recover_canonical(&bytes, PackagePolicyRecoveryLimits::default())
            .expect("strict recovery of the policy review just published");
    assert_eq!(
        recovered.canonical_bytes().expect("re-encode policy"),
        bytes
    );
    recovered
}

#[test]
fn trait_requirement_contract_binders_recover_in_the_declared_static_telescope() {
    // `StepContract::step<const Count>` states `requires value <= 37 && Count <= 37`,
    // so its contract names the requirement's own static binder. A trait has no
    // implicit subject binder before its declared statics.
    let recovered = reviewed_and_recovered(
        &kept_fixture("pass/facts/static_callback_reach_boundary_schema/main.omg"),
        "linux_x86_64",
    );
    let contract = recovered
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "StepContract")
        .expect("public StepContract");
    assert!(contract.type_parameters().is_empty());
    let [step] = contract.requirements() else {
        panic!("one StepContract requirement");
    };
    assert_eq!(step.type_parameters().len(), 1);
    assert!(!step.contracts().is_empty());
}

#[test]
fn toolchain_boundary_applications_recover_against_their_owner_qualified_requirement() {
    use review_evidence::encoding::PackagePolicyRecoveryLimits;
    use review_evidence::record::PackagePolicyBaseline;
    use review_evidence::record::PackageReviewNominalOwner;

    // `value + 1.0` closes an application of the toolchain `Float::add`
    // overload, whose requirement identity typed Psi qualifies with the
    // toolchain owner namespace.
    for fixture in [
        "pass/float/live_float_parameter_across_call_exit/main.omg",
        "run/float/canonical_arithmetic/main.omg",
    ] {
        let recovered = reviewed_and_recovered(&kept_fixture(fixture), "linux_x86_64");
        let add = recovered
            .boundary_applications()
            .realizations()
            .iter()
            .find(|realization| realization.operator_coordinate().identity().path() == "Float::add")
            .expect("closed Float::add application");
        assert!(matches!(
            add.operator_coordinate().identity().owner(),
            PackageReviewNominalOwner::ToolchainSource(_)
        ));
        assert!(
            add.requirement_identity()
                .contains(r"nominal\(toolchain-owner\,path\(Float::add\)\)"),
            "{}",
            add.requirement_identity()
        );
        for application in recovered.boundary_applications().realizations() {
            let plan =
                &recovered.selected_providers().plans()[application.selected_plan_index() as usize];
            assert_eq!(plan.target(), "linux_x86_64");
            assert_eq!(
                plan.schema_declaration(),
                application.operator_coordinate().identity()
            );
        }

        // Recovery must still join the exact emitted owner and deployment. Change
        // one copy in the real capsule, leaving the corresponding plan unchanged.
        let bytes = recovered.canonical_bytes().unwrap();
        let PackageReviewNominalOwner::ToolchainSource(owner) =
            add.operator_coordinate().identity().owner()
        else {
            unreachable!()
        };
        let digest = owner.digest();
        let position = bytes
            .windows(digest.len())
            .rposition(|window| window == digest)
            .unwrap();
        let mut substituted_owner = bytes.clone();
        substituted_owner[position] ^= 1;
        assert!(
            PackagePolicyBaseline::recover_canonical(
                &substituted_owner,
                PackagePolicyRecoveryLimits::default()
            )
            .is_err(),
            "a substituted toolchain owner must not recover"
        );

        let target = b"linux_x86_64";
        let position = bytes
            .windows(target.len())
            .rposition(|window| window == target)
            .unwrap();
        let mut substituted_target = bytes;
        substituted_target[position..position + target.len()].copy_from_slice(b"macos_x86_64");
        assert!(
            PackagePolicyBaseline::recover_canonical(
                &substituted_target,
                PackagePolicyRecoveryLimits::default()
            )
            .is_err(),
            "a different target must not recover under the Linux policy"
        );
    }
}

#[test]
fn numeric_library_review_does_not_deploy_unused_calls() {
    let source = platform_custody::temporary_directory::TemporaryDirectory::create(
        "unused-numeric-review",
        false,
    )
    .unwrap();
    std::fs::write(
        source.path().join("main.omg"),
        "public machine calculate(value: f64) -> f64 { value + 1.0 }\n",
    )
    .unwrap();
    std::fs::write(
        source.path().join("build.omg"),
        "machine build(builder: &mut Build) { Build::package(builder, \"unused_numeric\"); }\n",
    )
    .unwrap();
    let recovered = reviewed_and_recovered(&source.path().join("main.omg"), "uefi_x86_64");
    assert!(recovered.boundary_applications().realizations().is_empty());
    assert!(recovered.semantic_dependencies().iter().any(|dependency| {
        dependency.dependency().path() == "Float::add"
            && matches!(
                dependency.dependency().owner(),
                review_evidence::record::PackageReviewNominalOwner::ToolchainSource(_)
            )
    }));
}

#[test]
fn numeric_review_retains_calls_inside_a_selected_provider() {
    let recovered = reviewed_and_recovered(
        &kept_fixture("run/float/reviewed_provider/main.omg"),
        "linux_x86_64",
    );
    assert!(
        recovered
            .boundary_applications()
            .realizations()
            .iter()
            .any(|application| {
                application.operator_coordinate().identity().path() == "Float::add"
            }),
        "an arithmetic call reached only through a provider is still deployed"
    );
}

#[test]
fn opaque_realization_publishes_the_ceiling_it_inherits_from_its_requirement() {
    use review_evidence::record::PackageReviewForeignLocator;

    // The Build supplies the exact requirement without declaring a machine
    // body. Its checked calling application and inherited ceiling both survive.
    let fixture = kept_fixture("pass/boundary/native_provider_unoffered_read/main.omg");
    for (target, version, calling_policy) in [
        (
            "linux_x86_64",
            b"GLIBC_2.2.5".as_slice(),
            calling_conventions::CallingPolicy::SystemVAMD64,
        ),
        (
            "linux_arm64",
            b"GLIBC_2.17".as_slice(),
            calling_conventions::CallingPolicy::Aapcs64,
        ),
    ] {
        let recovered = reviewed_and_recovered(&fixture, target);
        assert!(recovered.selected_providers().plans().is_empty());
        let [binding] = recovered.native_bindings() else {
            panic!("one exact native Build binding for {target}");
        };
        let method = binding.method();
        assert_eq!(method.requirement().path(), "Probe::read");
        assert_eq!(method.requirement_owner().path(), "Probe");
        assert!(method.may_block());
        assert!(
            matches!(binding.locator(), Some(PackageReviewForeignLocator::ElfVersioned {
            object, symbol, version: actual,
        }) if object == b"libc.so.6" && symbol == b"read" && actual == version)
        );
        let [application] = method.calling() else {
            panic!("one validated calling application for {target}");
        };
        assert_eq!(application.requirement(), method.requirement());
        assert_eq!(application.native_parameters().len(), 3);
        assert_eq!(application.physical().parameters().len(), 3);
        assert_eq!(application.physical().policy(), calling_policy);
        assert_recovery_rejects_swapped_native_abi(&recovered, method);
        let requirement = recovered
            .callables()
            .callables()
            .iter()
            .find(|callable| callable.identity() == method.requirement())
            .expect("the original public bodyless requirement");
        assert_eq!(requirement.declared_may_block(), Some(true));
        assert!(!requirement.checked_may_block());
        assert!(requirement.checked_service_reach().realized().is_none());
        let declared = requirement.declared_service_reach().unwrap();
        assert_eq!(declared, std::slice::from_ref(method.requirement_owner()));
    }
}

fn assert_recovery_rejects_swapped_native_abi(
    policy: &review_evidence::record::PackagePolicyBaseline,
    method: &review_evidence::record::PackagePolicyServiceMethod,
) {
    use review_evidence::encoding::PackagePolicyRecoveryLimits;
    use review_evidence::record::PackagePolicyBaseline;
    use semantic_vocabulary::CallingAbiIdentity;

    fn encode_closed_abi(abi: &CallingAbiIdentity) -> Vec<u8> {
        assert!(abi.operands.is_empty());
        let mut bytes = abi.id().get().to_le_bytes().to_vec();
        for field in [&abi.requirement_identity, &abi.declaring_application] {
            bytes.extend_from_slice(&(field.len() as u64).to_le_bytes());
            bytes.extend_from_slice(field.as_bytes());
        }
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes
    }

    let [application] = method.calling() else {
        panic!("one validated calling application");
    };
    let abi = application.abi();
    let original = policy.canonical_bytes().unwrap();
    let key = encode_closed_abi(abi);
    let matches = original
        .windows(key.len())
        .enumerate()
        .filter(|(_, bytes)| *bytes == key)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let [offset] = matches.as_slice() else {
        panic!("one encoded ABI for Probe::read");
    };
    let mut wrong_parameter = abi.clone();
    wrong_parameter.requirement_identity = abi.requirement_identity.replacen("i32", "u32", 1);
    let mut wrong_result_dispatch = abi.clone();
    wrong_result_dispatch.requirement_identity = abi.requirement_identity.replacen(
        "result-dispatch()",
        "result-dispatch(arithmetic:saturating)",
        1,
    );
    let mut wrong_application = abi.clone();
    wrong_application.declaring_application =
        abi.declaring_application
            .replacen("arguments()", "arguments(named(name(u64)))", 1);
    let mut legacy = abi.clone();
    legacy.requirement_identity = application.requirement().path().to_owned();
    legacy.declaring_application = application.boundary_trait().path().to_owned();
    for forged in [
        wrong_parameter,
        wrong_result_dispatch,
        wrong_application,
        legacy,
    ] {
        assert_ne!(forged.id(), abi.id());
        // Recompute the coordinate too, so a matching checksum cannot conceal
        // disagreement with the recovered method telescope.
        let mut bytes = original.clone();
        bytes.splice(*offset..*offset + key.len(), encode_closed_abi(&forged));
        let error = PackagePolicyBaseline::recover_canonical(
            &bytes,
            PackagePolicyRecoveryLimits::default(),
        )
        .expect_err("a canonical ABI key must describe the recovered method");
        assert_eq!(
            error.to_string(),
            "package policy contains an invalid field value"
        );
    }
}

#[test]
fn review_recovery_retains_native_build_intents_without_runtime_demand() {
    let fixture = kept_fixture("pass/boundary/native_provider_unoffered_read/main.omg");
    let workspace = platform_custody::temporary_directory::TemporaryDirectory::create(
        "review-unused-native-binding",
        false,
    )
    .unwrap();
    let main = std::fs::read_to_string(&fixture).unwrap();
    let original_service = "public boundary trait Probe";
    assert_eq!(main.matches(original_service).count(), 1);
    let main = main.replace(original_service, "boundary trait Probe") + "\nmachine idle() {}\n";
    let build = std::fs::read_to_string(fixture.with_file_name("build.omg")).unwrap();
    let original_entry = "ProgramEntry, entry_start";
    assert_eq!(build.matches(original_entry).count(), 2);
    let root = workspace.path().join("main.omg");
    std::fs::write(&root, main).unwrap();
    std::fs::write(
        root.with_file_name("build.omg"),
        build.replace(original_entry, "ProgramEntry, idle"),
    )
    .unwrap();

    // The two original Build choices execute even though this entry never
    // calls Probe. Review retains the binding for the requested profile.
    let policy = reviewed_and_recovered(&root, "linux_x86_64");
    let [binding] = policy.native_bindings() else {
        panic!("one evaluated binding for the reviewed target");
    };
    assert_eq!(binding.method().requirement().path(), "Probe::read");
    assert!(binding.method().may_block());
    assert!(
        policy
            .public_traits()
            .iter()
            .all(|shape| shape.identity().path() != "Probe")
    );
    let [application] = binding.method().calling() else {
        panic!("the unused binding retains one exact calling application");
    };
    assert_eq!(
        application.physical().policy(),
        calling_conventions::CallingPolicy::SystemVAMD64
    );
}

#[test]
fn completed_scalar_call_preserves_wrapping_index_guard() {
    check_wrapping_index_guards(&[
        (
            "pass/facts/compound_index_guard_through_call_result_exit",
            true,
        ),
        (
            "fail/facts/compound_index_guard_through_call_result_rejected",
            false,
        ),
        ("fail/facts/compound_index_guard_wrong_call_actual", false),
    ]);
}

#[test]
fn match_join_preserves_wrapping_index_guard() {
    check_wrapping_index_guards(&[
        (
            "pass/facts/compound_index_guard_through_match_join_exit",
            true,
        ),
        (
            "fail/facts/compound_index_guard_through_match_join_rejected",
            false,
        ),
    ]);
}

fn check_wrapping_index_guards(fixtures: &[(&str, bool)]) {
    let target = "linux_x86_64";
    let mut failures = Vec::new();
    for &(fixture, accepted) in fixtures {
        let source = kept_fixture(fixture).join("main.omg");
        let result = orchestration::check_source(
            &mut CompilationSession::default(),
            CheckedCompileRequest::new(&source, Some(target)),
        )
        .result
        .map_err(|failure| failure.into_diagnostics());
        match result {
            Ok(checked) if accepted => {
                let publication = checked
                    .terminal_publication()
                    .published()
                    .expect("the checked fixture retains its Psi publication");
                let bytes = publication.artifact().to_bytes();
                let reopened = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes)
                    .expect("reopen the checked fixture's saved Psi");
                assert_eq!(
                    reopened.manifest().semantic(),
                    publication.artifact().manifest().semantic(),
                    "{fixture}"
                );
                eprintln!("SOURCE_CHECK {fixture}: accepted and reopened");
            }
            Err(diagnostics)
                if !accepted
                    && diagnostics.iter().any(|diagnostic| {
                        diagnostic.message.contains("cannot prove index")
                            && diagnostic.message.contains("within length 200")
                    }) =>
            {
                eprintln!("SOURCE_CHECK {fixture}: intended bound refusal {diagnostics:?}")
            }
            Ok(_) => failures.push(format!("{fixture}: invalid index source accepted")),
            Err(diagnostics) => failures.push(format!("{fixture}: {diagnostics:?}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// The foreign contract keeps its authored method identity, while its closed
// outer and local arguments control exactly which call instances it can supply.
// Restrict an existing working registrar to the callback it actually receives.
// Its program, C provider, layout and exit oracle remain the original control.
#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn closed_native_binding_reloads_and_executes_the_descriptor_callback() {
    use std::sync::Arc;
    let root = fixture_package_inputs::repo_root();
    let fixture = kept_fixture("pass/layouts/private_callback_inherited_policy_exit");
    let scratch = root.join("build").join(format!(
        "omega-closed-native-callback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&scratch).unwrap();
    for entry in std::fs::read_dir(&fixture).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            std::fs::copy(entry.path(), scratch.join(entry.file_name())).unwrap();
        }
    }
    assert_eq!(
        std::fs::read(scratch.join("main.omg")).unwrap(),
        std::fs::read(fixture.join("main.omg")).unwrap()
    );
    let original = std::fs::read_to_string(scratch.join("build.omg")).unwrap();
    assert_eq!(
        original.matches("Build::bind<Registrar::register>").count(),
        1
    );
    let build = original
        .replace(
            "Build::bind<Registrar::register>",
            "Build::bind<Registrar::register<TimerHandler::call>>",
        )
        .replace(
            "location: \"../../../../../source/library/std\"",
            &format!("location: {:?}", root.join("source/library/std")),
        );
    std::fs::write(scratch.join("build.omg"), build).unwrap();
    let provider = scratch.join("provider");
    let prepared_provider = std::process::Command::new("python3")
        .arg(fixture.join("fixture/prepare.py"))
        .arg("--output")
        .arg(&provider)
        .output()
        .expect("prepare the existing authored C provider");
    assert!(
        prepared_provider.status.success(),
        "C provider: {:?}",
        prepared_provider
    );
    eprintln!(
        "C_PROVIDER_PREPARATION {}",
        String::from_utf8_lossy(&prepared_provider.stdout)
    );
    let source = scratch.join("main.omg");
    let discovered = fixture_package_inputs::discover_fixture(
        &source,
        package_compilation::BuildExecutionInputs {
            snapshot: fixture_package_inputs::fixture_build_snapshot(&source).unwrap(),
            ..Default::default()
        },
    )
    .expect("discover the exact closed registrar binding");
    let product =
        compiler::CompilationProduct::Native(products::NativeProductRequest {
            installation_assumptions: Vec::new(),
            retain_native_inputs: true,
            image: Default::default(),
            private_entry_ram: Vec::new(),
            terminal_authority_policy: terminal_psi_to_abstract_operations::provider_admission::current_terminal_authority_policy(),
            terminal_authority_permission_policy: None,
            optimization_rollback: Default::default(),
        });
    let mut prepared = fixture_package_inputs::review_fixture(
        discovered,
        &[target::TargetProfile::MacosArm64],
        product,
    )
    .expect("admit the closed native product without weakening its callback contract");
    // Stop at the ordinary package producer's pending native result so saved
    // inputs can be recovered before native completion consumes them.
    assert_eq!(prepared.plans.len(), 1);
    let (profile, request) = prepared.plans.pop().unwrap();
    assert_eq!(profile, target::TargetProfile::MacosArm64);
    let root_occurrence = request.occurrences.len() - 1;
    let build = orchestration::prepare_package(
        prepared.session.compiler(),
        request.occurrences,
        &mut prepared.completed,
    )
    .unwrap_or_else(|error| {
        panic!(
            "prepare the admitted package's exact source closure at occurrence {}: {:?}",
            error.occurrence,
            error.failure.into_diagnostics()
        )
    });
    let mut outcomes = prepared
        .session
        .compiler()
        .complete_package_products(vec![(
            (),
            compiler::PreparedPackageProduct {
                build,
                product: request.product,
                permissions: request.terminal_permissions,
            },
        )])
        .expect("produce the native product and saved inputs");
    assert_eq!(outcomes.len(), 1);
    let ((), output) = outcomes.pop().unwrap();
    let mut output = output.expect("produce the admitted native application");
    let compiler::CompilationOutput::Native(pending) = &output else {
        panic!("expected native application work");
    };
    let bytes = pending
        .retained_native_inputs
        .as_ref()
        .expect("save the exact native derivation inputs");
    let artifact = Arc::new(
        terminal_codec::CanonicalTerminalArtifact::from_bytes(
            &pending.terminal.artifact().to_bytes(),
        )
        .unwrap(),
    );
    let (recovered, recovered_permissions, installation_assumptions) =
        terminal_psi_to_abstract_operations::recover_native_derivation_inputs(
            {
                let authorization = pending.terminal.authorization().clone();
                authorization.require_artifact(&artifact).unwrap();
                authorization
            },
            bytes,
        )
        .expect("reload the closed binding from its canonical artifact and saved inputs");
    assert_eq!(recovered, pending.terminal);
    assert_eq!(recovered_permissions, pending.accepted_package_permissions);
    assert!(installation_assumptions.is_empty());
    // Keep the already retained coordinates with a failure so a missing choice
    // can be distinguished from a different closed call without another probe.
    let proposal = recovered.native_realization_proposal().unwrap();
    let module = recovered.artifact().semantic_module();
    let mut binding_context = format!(
        "target={} selected_plans={} direct_leaves={} callbacks={}\n",
        proposal.target_profile().target_name(),
        proposal.providers().selection.plans().len(),
        proposal.providers().direct_boundary_leaves.len(),
        module.callback_applications.len(),
    );
    for callback in &module.callback_applications {
        let operation = module
            .operations()
            .find(|operation| operation.id == callback.registrar)
            .expect("retained callback registrar operation");
        let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = operation.kind else {
            panic!("retained registrar must be a boundary call");
        };
        let declaration = module.boundary_machine(boundary).unwrap();
        let abi = module.calling_abi_identity(boundary).unwrap();
        binding_context.push_str(&format!(
            "registrar={:?} boundary={boundary:?} owner={:?} required={:?} abi={abi:?}\n",
            callback.registrar,
            module.identities.text(declaration.owner.identity()),
            module.identities.text(declaration.identity),
        ));
        for plan in proposal.providers().selection.plans() {
            for row in plan.rows.iter().filter(|row| {
                plan.identities.text(row.declared_requirement_identity) == abi.requirement_identity
            }) {
                binding_context.push_str(&format!(
                    "selected target={} root={:?} selected={:?} declared={:?} abi={:?}\n",
                    plan.target,
                    plan.schema.root_application_identity(),
                    plan.identities.text(row.requirement_identity),
                    plan.identities.text(row.declared_requirement_identity),
                    row.calling_signature,
                ));
            }
        }
    }
    for target in proposal.target_provider_proposals() {
        match &target.selection {
            Ok(selection) => {
                binding_context.push_str(&format!(
                    "publication target={} plans={}\n",
                    target.target,
                    selection.plans.len()
                ));
                for selected in &selection.plans {
                    for row in &selected.rows {
                        if row.calling_signature.is_some() {
                            binding_context.push_str(&format!(
                                "publication selected={:?} declared={:?} abi={:?}\n",
                                selected.identities.text(row.requirement_identity),
                                selected.identities.text(row.declared_requirement_identity),
                                row.calling_signature,
                            ));
                        }
                    }
                }
            }
            Err(errors) => binding_context.push_str(&format!(
                "publication target={} refused={errors:?}\n",
                target.target
            )),
        }
    }
    let context_path = scratch.join("native-binding-context.txt");
    std::fs::write(&context_path, &binding_context).unwrap();
    let source_selection = proposal.providers().selection.clone();
    // Std can retain unrelated direct imports. Keep these source-free records
    // so the final reached registrar can still be checked for an extra stub.
    let direct_boundary_leaves = proposal.providers().direct_boundary_leaves.clone();
    // Native completion now has only recovered semantic inputs, not a source
    // compiler that could supply a missing declaration or calling decision.
    drop(std::mem::take(prepared.session.compiler()));
    drop(std::mem::take(&mut prepared.completed));
    let compiler::CompilationOutput::Native(pending) = &mut output else {
        unreachable!()
    };
    pending.terminal = recovered;
    pending.accepted_package_permissions = recovered_permissions;
    let completed =
        orchestration::complete_product(output).map_err(|error| compiler::PackageCompileFailure {
            occurrence: root_occurrence,
            failure: error.into_diagnostics().into(),
        });
    let mut outcomes = prepared
        .completion
        .take()
        .expect("the original package admission must still close its outcome")
        .complete(vec![completed], prepared.session.resources());
    assert_eq!(outcomes.len(), 1);
    let outcome = outcomes.pop().unwrap();
    assert_eq!(outcome.target, profile);
    let report = outcome
        .result
        .unwrap_or_else(|error| {
            panic!(
                "complete recovered native work through ordinary package reconciliation: {error:?}\nretained native binding coordinates: {}\n{binding_context}",
                context_path.display()
            )
        })
        .into_report()
        .expect("complete actual native lowering after saved-input recovery");
    drop(prepared);
    // The emitted call must use the exact saved native row. Its checked scope
    // and completed image also show that this callback was actually reached.
    let artifact = report.retained_native_artifact().unwrap();
    let module = artifact.psi_artifact().semantic_module();
    let scope = artifact
        .object()
        .native_execution_scope(
            artifact.psi_artifact(),
            *artifact.selected_provider_closure_digest().as_bytes(),
        )
        .expect("rejoin the actual image's artifact, target, entry and provider choices")
        .expect("the checked native image must retain its execution scope");
    assert!(
        !scope.callbacks().is_empty(),
        "the existing descriptor callback must actually be selected for execution"
    );
    for occurrence in scope.callbacks() {
        let callback = module
            .callback_application(occurrence.application_index())
            .expect("the executed callback must retain its exact application");
        assert_eq!(callback.registrar, occurrence.terminal_operation());
        let operation = module
            .operations()
            .find(|operation| operation.id == callback.registrar)
            .expect("actual emitted registrar operation");
        let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = operation.kind else {
            panic!("registrar must retain its exact boundary call");
        };
        assert!(scope.contains_boundary(boundary));
        let declaration = module
            .boundary_machines
            .iter()
            .find(|declaration| declaration.id == boundary)
            .unwrap();
        let requirement = module.identities.text(declaration.identity).unwrap();
        let source = &declaration
            .declaration
            .as_ref()
            .expect("the executed registrar retains its checked declaration")
            .source;
        let semantic = module
            .identities
            .text(source.semantic_declaration)
            .expect("the registrar has its canonical semantic declaration");
        let callable = module
            .identities
            .text(source.callable)
            .expect("the registrar has its complete checked callable");
        assert!(
            direct_boundary_leaves.iter().all(|leaf| {
                !leaf
                    .plan
                    .rows
                    .iter()
                    .any(|row| leaf.plan.identities.text(row.requirement_identity) == requirement)
                    && !(leaf.declaration_identity == semantic
                        && leaf.callable_identity == callable)
            }),
            "the reached bare native registrar must not acquire a forwarding-machine leaf"
        );
        let (plan, row) = source_selection
            .selected_requirement_row(requirement)
            .expect("the reached closed registrar must retain its exact native binding");
        assert_ne!(row.requirement_identity, row.declared_requirement_identity);
        let selected = artifact
            .selected_provider_plans()
            .iter()
            .filter(|plan| {
                plan.requirement_identities()
                    .iter()
                    .any(|id| id == requirement)
            })
            .collect::<Vec<_>>();
        let [selected] = selected.as_slice() else {
            panic!(
                "executed registrar {requirement} must select exactly one completed plan; got {}",
                selected.len()
            );
        };
        assert_eq!(selected.report_identity(), plan.report_fingerprint());
        assert_eq!(
            selected.plan_digest().as_bytes(),
            plan.identity_digest().as_bytes()
        );
        let abi = module.calling_abi_identity(boundary).unwrap();
        assert_eq!(row.calling_signature, Some(abi.id()));
        assert_eq!(
            plan.identities.text(row.declared_requirement_identity),
            abi.requirement_identity
        );
        let matching = plan
            .schema
            .calling_applications
            .iter()
            .enumerate()
            .filter(|(_, application)| application.abi == abi)
            .collect::<Vec<_>>();
        let [(calling_index, original)] = matching.as_slice() else {
            panic!("the exact ABI must retain exactly one original checked boundary contract");
        };
        assert_eq!(plan.schema.calling_application(&abi), Some(&original.plan));
        assert_eq!(
            plan.schema.identity_text(original.requirement),
            abi.requirement_identity
        );
        assert!(!original.plan.commitment.is_zero());
        assert_ne!(original.plan.report_fingerprint, 0);
        let wrong = module
            .calling_abi_signatures
            .iter()
            .find(|declaration| {
                declaration.id != abi.id()
                    && module.identities.text(declaration.requirement_identity)
                        != Some(abi.requirement_identity.as_str())
            })
            .expect("another actual declaration ABI must provide a substitution falsifier")
            .identity(&module.identities)
            .unwrap();
        assert!(plan.schema.calling_application(&wrong).is_none());
        let mut changed = plan.schema.clone();
        // The original requirement coordinate must reject another real
        // declaration's ABI, independently of its supplied calling policy.
        changed.calling_applications[*calling_index].abi = wrong;
        assert_eq!(
            changed.validate_calling_applications().unwrap_err(),
            "calling evidence ABI key differs from its requirement coordinate"
        );
    }
    native_entry_controls::inspect(&report, &scratch)
        .expect("preserve the original native entry readback falsifiers");
    let published =
        products::publication::publish_compilation(report, &scratch.join("native-output"))
            .expect("publish the checked native executable");
    let observed = std::process::Command::new(published.checked_native_executable_path().unwrap())
        .env("DYLD_LIBRARY_PATH", &provider)
        .output()
        .expect("run the unchanged descriptor callback program");
    assert_eq!(
        observed.status.code(),
        Some(0),
        "native callback oracle: {observed:?}"
    );
    std::fs::remove_dir_all(scratch).unwrap();
}

/// Use a genuine accepted constant invocation; cleanup must retain its cost
/// even though the successful caller result contains no receipt fields.
fn build_constant_cleanup_history() -> products::SourceCompletionAccounting {
    let text =
        "const VALUE:u64=answer(); machine answer()->u64 {4u64} machine build()->u64 {VALUE}";
    let mut sources = source::SourceMap::default();
    let id = sources
        .add_checked_instance(
            "/unit/build.omg".into(),
            text,
            "/unit".into(),
            None,
            source::SourceOrigin::Toolchain,
            source::SourceResolutionStratum::Base,
            source::DependencyScope::Build,
        )
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap();
    let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
    request.sources = Some(std::sync::Arc::new(sources));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let entry = typed.realized_machine_named("build").unwrap().symbol;
    let prepared = source_compiler::SourcePreparationRequest {
        program: typed,
        accounting: Default::default(),
        admission: Default::default(),
        compilation: source_compiler::TerminalSourceCompilation {
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Build,
            selected: &[],
            declaration_metadata: &[],
            bindings: &[],
            supplies: &[],
        },
    }
    .prepare()
    .unwrap();
    assert_eq!(prepared.accounting.receipts().len(), 1);
    prepared.accounting.validate_charges().unwrap();
    prepared.accounting
}

fn force_build_cleanup_failure(workspace: &std::path::Path) -> std::path::PathBuf {
    let staging = std::fs::read_dir(workspace)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::remove_dir_all(&staging).unwrap();
    std::fs::write(&staging, b"not a staging directory").unwrap();
    staging
}

#[test]
fn build_constant_cleanup_failure_retains_successful_execution_costs() {
    use package_manager::review::{
        CompileResolvedPackageReviewCause as Cause, CompileResolvedPackageReviewsError as Error,
    };
    let history = build_constant_cleanup_history();
    let workspace = platform_custody::temporary_directory::TemporaryDirectory::create(
        "constant-cleanup-success",
        false,
    )
    .unwrap();
    let mut session = package_manager::PackageBuildSession::create(workspace.path()).unwrap();
    session.record_source_completion(history.clone());
    session.record_source_completion(history);
    let staging = force_build_cleanup_failure(workspace.path());
    let failure = session.dispose::<_, Error>(Ok(())).unwrap_err();
    assert!(matches!(failure.cause, Cause::BuildStagingCleanup { .. }));
    let accounting = failure.source_completion_accounting().unwrap();
    assert_eq!(
        accounting.receipts().len(),
        1,
        "held history is not a second execution"
    );
    accounting.validate_charges().unwrap();
    std::fs::remove_file(staging).unwrap();
}

#[test]
fn build_constant_cleanup_failure_preserves_the_primary_typed_cause() {
    let history = build_constant_cleanup_history();
    let workspace = platform_custody::temporary_directory::TemporaryDirectory::create(
        "constant-cleanup-primary",
        false,
    )
    .unwrap();
    let mut session = orchestration::BuildSession::create(workspace.path()).unwrap();
    session.resources().record_source_completion(history);
    let staging = force_build_cleanup_failure(workspace.path());
    let primary = package_manager::operations::PackageInspectionError::from(
        products::CompileFailure::Diagnostics(vec![diagnostics::Diagnostic::error(
            "actual primary inspection failure",
        )]),
    );
    assert!(primary.source_completion_accounting().receipts().is_empty());
    let failure = session.dispose_inspection::<()>(Err(primary)).unwrap_err();
    let Some(products::CompileFailure::Diagnostics(diagnostics)) = failure.compilation_failure()
    else {
        panic!("cleanup must preserve the original typed compilation cause")
    };
    assert_eq!(
        diagnostics::join_messages(diagnostics),
        "actual primary inspection failure"
    );
    assert_eq!(failure.source_completion_accounting().receipts().len(), 1);
    std::fs::remove_file(staging).unwrap();
}

fn captured_build_constant_session(
    case: &str,
) -> (
    platform_custody::temporary_directory::TemporaryDirectory,
    orchestration::BuildSession,
    Result<orchestration::ResolvedProjectBuilds, orchestration::ProjectSourceResolutionError>,
) {
    let workspace = platform_custody::temporary_directory::TemporaryDirectory::create(
        "captured-build-constant",
        false,
    )
    .unwrap();
    let project = workspace.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/build_constant_accounting")
        .join(case);
    for name in ["main.omg", "build.omg"] {
        std::fs::copy(fixture.join(name), project.join(name)).unwrap();
    }
    let profile = target::TargetProfile::host_if_supported().expect("host Build execution target");
    let preparation = package_manager::operations::begin_local_project_preparation(
        &project.join("main.omg"),
        &[profile],
        true,
    )
    .unwrap()
    .unwrap();
    let mut session =
        orchestration::BuildSession::create(&workspace.path().join("session")).unwrap();
    let outcome = orchestration::resolve_project_sources(
        &mut session,
        orchestration::ProjectSourceRequest {
            preparation,
            collect_timings: false,
            execution: package_compilation::BuildExecutionInputs {
                profile: Some(profile),
                ..Default::default()
            },
        },
    );
    (workspace, session, outcome)
}

#[test]
fn build_constant_captured_source_cost_survives_real_readiness_and_cleanup() {
    let (workspace, mut session, outcome) = captured_build_constant_session("positive");
    let resolved = outcome.unwrap_or_else(|error| panic!("captured positive Build: {error}"));
    assert!(
        !session
            .resources()
            .source_completion_accounting()
            .receipts()
            .is_empty(),
        "actual readiness must retain the computed constant work"
    );
    let profile = resolved.execution_profile;
    for build in resolved.builds {
        let inputs = build.package_inputs().clone();
        let product = build
            .complete(session.compiler(), profile, inputs)
            .unwrap_or_else(|failure| {
                panic!(
                    "captured Build Product rejoin: {}",
                    diagnostics::join_messages(&failure.into_diagnostics())
                )
            });
        // The package coordinator retains this actual prepared owner before
        // consuming it for a target product; use that same published ledger.
        session
            .resources()
            .record_source_completion(product.source_completion_accounting().clone());
        let checked = session
            .compiler()
            .check_package_build(product)
            .unwrap_or_else(|failure| {
                panic!(
                    "captured Product source: {}",
                    diagnostics::join_messages(&failure.into_diagnostics())
                )
            });
        let accounting = checked.source_completion_accounting();
        accounting.validate_charges().unwrap();
        let invocations = accounting
            .receipts()
            .iter()
            .filter_map(|record| match record {
                build_time_evaluation::PureEvaluationRecord::Invocation(receipt)
                    if receipt.request_source().symbols.name(receipt.requester()) == "VALUE" =>
                {
                    Some(receipt.request_source().identity.get())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            invocations.len(),
            2,
            "the retyped Build must evaluate its own constant"
        );
        assert_ne!(
            invocations[0], invocations[1],
            "the new Build cannot borrow the earlier source receipt"
        );
        session
            .resources()
            .record_source_completion(accounting.clone());
    }
    let history = session.resources().source_completion_accounting().clone();
    history.validate_charges().unwrap();
    let staging = force_build_cleanup_failure(&workspace.path().join("session"));
    let primary = package_manager::operations::PackageInspectionError::from(
        "inspection finish refused after Build execution".to_owned(),
    );
    assert!(primary.source_completion_accounting().receipts().is_empty());
    let failure = session.dispose_inspection::<()>(Err(primary)).unwrap_err();
    assert_eq!(
        failure.to_string(),
        "inspection finish refused after Build execution"
    );
    assert_eq!(
        failure.source_completion_accounting().receipts().len(),
        history.receipts().len()
    );
    failure
        .source_completion_accounting()
        .validate_charges()
        .unwrap();
    std::fs::remove_file(staging).unwrap();
}

#[test]
fn build_constant_captured_invalid_twin_rejects_its_original_call_obligation() {
    let (_workspace, mut session, outcome) = captured_build_constant_session("negative");
    let failure = match outcome {
        Err(error) => error,
        Ok(_) => panic!("bounded(9) requires 9 <= 5"),
    };
    let message = failure.to_string();
    assert!(message.contains("bounded"), "{message}");
    assert!(message.contains("9 <= 5"), "{message}");
    session.close().unwrap();
}

const REVIEW_BUILD_SOURCE: &str = r#"
machine build(builder: &mut Build) {
    Build::application(builder, "accepted_package_review");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
    Build::bind(builder, linux_arm64::ProgramEntry, main);
    Build::bind(builder, macos_arm64::ProgramEntry, main);
    Build::bind(builder, windows_x86_64::ProgramEntry, main);
}
"#;

fn captured_review_compilation(
    main_source: &str,
    build_source: &str,
) -> (
    platform_custody::temporary_directory::TemporaryDirectory,
    orchestration::BuildSession,
    Result<products::CheckedCompilation, Vec<diagnostics::Diagnostic>>,
) {
    let workspace = platform_custody::temporary_directory::TemporaryDirectory::create(
        "accepted-package-review",
        false,
    )
    .unwrap();
    let (session, checked) = captured_review_compilation_at(
        &workspace.path().join("project"),
        &workspace.path().join("session"),
        main_source,
        build_source,
    );
    (workspace, session, checked)
}

// Each revision gets a fresh accepted Product while its package keeps the
// same project identity. Reusing a judgment would hide declaration changes.
fn captured_review_compilation_at(
    project: &std::path::Path,
    session_path: &std::path::Path,
    main_source: &str,
    build_source: &str,
) -> (
    orchestration::BuildSession,
    Result<products::CheckedCompilation, Vec<diagnostics::Diagnostic>>,
) {
    std::fs::create_dir_all(project).unwrap();
    std::fs::write(project.join("main.omg"), main_source).unwrap();
    std::fs::write(project.join("build.omg"), build_source).unwrap();
    let target = target::TargetProfile::host_if_supported().expect("host Product target");
    let (mut session, build, inputs) =
        captured_package_build(&project.join("main.omg"), session_path, target, None);
    let checked = session
        .compiler()
        .check_package_build(build)
        .map_err(|failure| failure.into_diagnostics());
    if let Ok(checked) = &checked {
        assert_eq!(
            checked.sources().accepted_semantic_bindings(),
            inputs
                .accepted_semantic_bindings()
                .cloned()
                .collect::<Vec<_>>()
        );
    }
    (session, checked)
}

// Capture the exact authored project and finish its Build before checking or
// reviewing the Product. Review has no fixture-supplied execution authority.
fn captured_package_build(
    root_path: &Path,
    session_path: &Path,
    target: target::TargetProfile,
    snapshot: Option<package_compilation::BuildSnapshotRequest>,
) -> (
    orchestration::BuildSession,
    compiler::PackageBuild,
    package_compilation::PackageCompilationInputs,
) {
    let preparation =
        package_manager::operations::begin_local_project_preparation(root_path, &[target], true)
            .unwrap()
            .expect("ordinary captured project");
    let mut session = orchestration::BuildSession::create(session_path).unwrap();
    let resolved = orchestration::resolve_project_sources(
        &mut session,
        orchestration::ProjectSourceRequest {
            preparation,
            execution: package_compilation::BuildExecutionInputs {
                profile: target::TargetProfile::host_if_supported(),
                snapshot,
                ..Default::default()
            },
            collect_timings: false,
        },
    )
    .unwrap_or_else(|error| panic!("captured package Build: {error}"));
    let package = resolved.prepared.root_package();
    let build = resolved
        .builds
        .into_iter()
        .find(|build| build.package_inputs().root() == package)
        .expect("the root package's actual Build");
    let inputs = build.package_inputs().clone();
    let build = build
        .complete(session.compiler(), target, inputs.clone())
        .unwrap_or_else(|failure| panic!("Product preparation: {:?}", failure.into_diagnostics()));
    (session, build, inputs)
}

#[test]
fn review_keeps_the_target_selected_fixed_layout_delivery_contract() {
    use review_evidence::encoding::PackagePolicyRecoveryLimits;
    use review_evidence::record::PackagePolicyBaseline;

    let main = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../samples/cli/facts/fixed_layout_storage/target_body/main.omg"
    ));
    let build = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../samples/cli/facts/fixed_layout_storage/target_body/build.omg"
    ));
    let (_workspace, _session, checked) = captured_review_compilation(main, build);
    let checked = checked.expect("the original Product reaches checked target supply");
    let package = checked.sources().package_identity().unwrap();
    let target = checked.execution().target_profile().unwrap();
    let original = review_evidence::project_checked_package_policy(&checked, target, package)
        .expect("the exact selected local boundary is reviewed");
    let bytes = original.canonical_bytes().unwrap();
    let policy =
        PackagePolicyBaseline::recover_canonical(&bytes, PackagePolicyRecoveryLimits::default())
            .unwrap();
    assert_eq!(policy.canonical_bytes().unwrap(), bytes);
    let [plan] = policy.selected_providers().plans() else {
        panic!("one whole checked Delivery plan");
    };
    let expected = if target == target::TargetProfile::MacosArm64 {
        "FirstDelivery"
    } else {
        "SecondDelivery"
    };
    assert_eq!(plan.target(), target.target_name());
    assert_eq!(plan.provider_type(), expected);
    assert_eq!(plan.provider_type_declaration().unwrap().path(), expected);
    assert_eq!(plan.schema_declaration().path(), "Delivery");
    assert_eq!(plan.rows().len(), 2);
    let method = plan
        .methods()
        .iter()
        .find(|method| method.name() == "value")
        .unwrap();
    assert!(method.terminates_guarantee());
    let result = method.signature().result().unwrap().canonical();
    for identity in ["LayoutStorage", "Pair", "PairLayout"] {
        assert!(result.contains(identity), "{result}");
    }
    for row in plan.rows() {
        assert_eq!(
            row.realization().unwrap().path(),
            format!("{expected}::{}", row.method())
        );
    }
    assert!(policy.public_traits().is_empty(), "Delivery stays private");
    for row in plan.rows() {
        let requirement = policy
            .callables()
            .callables()
            .iter()
            .find(|callable| callable.identity() == row.requirement())
            .unwrap();
        assert_eq!(requirement.declared_service_reach(), Some(&[][..]));
        assert_eq!(
            format!("{:?}", requirement.checked_crash().interface()),
            "PublishedCeiling",
        );
        assert!(requirement.checked_crash().published().is_empty());
    }
    let foreign = semantic_vocabulary::PackageKeyIdentity::from_digest([42; 32]).unwrap();
    assert!(review_evidence::project_checked_package_policy(&checked, target, foreign).is_err());
    let other_target = if target == target::TargetProfile::MacosArm64 {
        target::TargetProfile::LinuxX64
    } else {
        target::TargetProfile::MacosArm64
    };
    assert!(
        review_evidence::project_checked_package_policy(&checked, other_target, package).is_err()
    );
}

#[test]
fn review_refuses_a_boundary_without_a_checked_provider_policy() {
    let source = "
        boundary trait Unrelated {
            machine unused() reaches terminates crashes none;
        }
        machine main() {}
    ";
    let (_workspace, _session, checked) = captured_review_compilation(source, REVIEW_BUILD_SOURCE);
    let checked = checked.expect("the unattached requirement can be checked");
    let package = checked.sources().package_identity().unwrap();
    let target = checked.execution().target_profile().unwrap();
    let errors = review_evidence::project_checked_package_policy(&checked, target, package)
        .expect_err("the unselected requirement has no complete review policy");
    let reason = diagnostics::join_messages(&errors);
    assert!(reason.contains("boundary"), "{reason}");
}

/// Review keeps which lifetime backs each public field. Renaming binders
/// preserves that relation; tying both fields to one input changes the API.
#[test]
fn accepted_package_review_keeps_public_data_lifetime_bindings() {
    let workspace = platform_custody::temporary_directory::TemporaryDirectory::create(
        "public-data-lifetime-review",
        false,
    )
    .unwrap();
    let project = workspace.path().join("project");
    let original = r#"
public data Text<'view> [copy] { bytes: &'view [u8]; }
public data Message<'first, 'second> [copy] {
    left: Text<'first>; right: &'second [u8];
}
machine main() {}
"#;
    let renamed = original
        .replace("'first", "'alpha")
        .replace("'second", "'beta");
    let rebound = renamed.replace("right: &'beta", "right: &'alpha");
    let mut package = None;
    let mut shapes = Vec::new();
    for (ordinal, source) in [original, renamed.as_str(), rebound.as_str()]
        .into_iter()
        .enumerate()
    {
        let (_session, checked) = captured_review_compilation_at(
            &project,
            &workspace.path().join(format!("session-{ordinal}")),
            source,
            REVIEW_BUILD_SOURCE,
        );
        let checked =
            checked.unwrap_or_else(|diagnostics| panic!("Product judgment: {diagnostics:?}"));
        let identity = checked.sources().package_identity().unwrap();
        if let Some(prior) = package {
            assert_eq!(
                prior, identity,
                "source revisions must keep the actual project identity"
            );
        } else {
            package = Some(identity);
        }
        let target = checked.execution().target_profile().unwrap();
        let reviewed =
            review_evidence::project_package_review(&checked, identity, target, usize::MAX)
                .unwrap_or_else(|error| panic!("public lifetime data review: {error:?}"));
        reviewed
            .policy
            .validate_package_membership(
                |package| package == identity,
                review_evidence::encoding::PackagePolicyMembershipLimits::default(),
            )
            .expect("declared field lifetimes must survive normal lock membership");
        assert_eq!(reviewed.policy.public_data().len(), 2);
        let shape = reviewed
            .policy
            .public_data()
            .iter()
            .find(|shape| shape.lifetime_parameter_count() == 2)
            .expect("the actual two-binder Message declaration must be retained");
        assert_eq!(shape.lifetime_parameter_count(), 2);
        assert_eq!(shape.members().len(), 2);
        shapes.push(shape.clone());
    }
    assert_eq!(
        shapes[0], shapes[1],
        "binder spelling must not change the data API"
    );
    assert_ne!(
        shapes[1], shapes[2],
        "field lifetime correspondence must remain visible"
    );
}

#[test]
fn accepted_package_review_keeps_declarations_and_refuses_detached_source() {
    use review_evidence::record::PackagePolicyBaseline;
    use review_evidence::{
        PackageReviewInput, project_checked_package_policy, project_package_review,
    };

    // These are the original public Pair/Delivered declarations from the
    // fixed-layout customer. This control isolates review from its still
    // independently developed layout/native realization. Build and checking
    // use the ordinary captured package route, with no fixture consent.
    const SOURCE: &str = r#"
public domain u64::Delivered requires self >= 42u64 && self <= 42u64;
public data Pair [copy] { lo: u64 in Delivered; hi: u64; }
machine main() {}
"#;
    let (workspace, mut session, checked) =
        captured_review_compilation(SOURCE, REVIEW_BUILD_SOURCE);
    let checked = checked.unwrap_or_else(|diagnostics| panic!("Product judgment: {diagnostics:?}"));
    let project = workspace.path().join("project");
    let package = checked.sources().package_identity().unwrap();
    let target = checked.execution().target_profile().unwrap();
    let input = PackageReviewInput::supplied(&checked, &checked).unwrap();
    assert!(std::ptr::eq(
        input.module().unwrap(),
        checked.terminal_publication().accepted().unwrap().module()
    ));
    let detached: typed_trees::TypedTrees = checked
        .source_inspection()
        .program()
        .as_ref()
        .as_ref()
        .clone();
    let errors = match PackageReviewInput::supplied(&detached, &checked) {
        Ok(_) => panic!("equal declarations cannot replace the judged source owner"),
        Err(errors) => errors,
    };
    assert!(diagnostics::join_messages(&errors).contains("differs from the accepted source owner"));

    // Review consumes retained source and judgment, even after the original
    // live files disappear. It must not parse the filesystem a second time.
    std::fs::remove_file(project.join("main.omg")).unwrap();
    std::fs::remove_file(project.join("build.omg")).unwrap();
    let reviewed = project_package_review(&checked, package, target, usize::MAX)
        .unwrap_or_else(|error| panic!("accepted package review: {error:?}"));
    assert_eq!(reviewed.policy.public_data().len(), 1);
    assert_eq!(reviewed.policy.public_domains().len(), 1);
    assert!(
        !reviewed.policy.public_domains()[0]
            .predicate_facts()
            .is_empty()
    );
    let [build] = reviewed.policy.callables().callables() else {
        panic!("the selected Build must retain its own checked contract");
    };
    assert_eq!(
        build.identity().path(),
        checked
            .symbols
            .display_path(checked.build().selected_build_machine_symbol.unwrap(), "::",)
    );
    assert_eq!(
        build.identity().owner(),
        review_evidence::record::PackageReviewNominalOwner::Package(package)
    );
    // The authored Build calls `application`, whose public contract reaches
    // and enters BuildRequestOperations. Review must retain that nonempty
    // contribution while preserving omission of Build's own written ceilings.
    let realized = build.checked_service_reach().realized().unwrap();
    let request = realized
        .iter()
        .find(|service| service.path() == "BuildRequestOperations")
        .expect("the actual Build request protocol must remain visible");
    assert!(matches!(
        request.owner(),
        review_evidence::record::PackageReviewNominalOwner::ToolchainSource(_)
    ));
    assert_eq!(build.checked_service_reach().concrete(), Some(realized));
    assert_eq!(build.service_reach_dependency().concrete(), realized);
    assert!(build.declared_service_reach().is_none());
    assert!(build.declared_synchronous_invocations().is_none());
    assert!(
        build
            .realized_synchronous_invocations()
            .iter()
            .any(|entry| entry.service() == Some(request))
    );
    assert_eq!(build.parameters().len(), 1);
    // A real Product judgment cannot stand in for the independently judged
    // Build invocation, and dropping Build custody cannot publish its rows.
    let source = std::sync::Arc::clone(checked.source_inspection().program());
    let roster = checked.terminal_publication().roster().unwrap().clone();
    let other_judgment = roster.judge(std::sync::Arc::clone(&source)).unwrap();
    let publication = std::sync::Arc::new(
        products::ProductTerminalPublication::judge(
            source,
            roster,
            checked.terminal_publication().optimizations().clone(),
        )
        .unwrap(),
    );
    let inspection = std::sync::Arc::new(products::SelectedProviderSource::new(
        std::sync::Arc::clone(checked.source_inspection().inventory()),
        std::sync::Arc::clone(checked.execution().shared_plans()),
    ));
    let rewrap = |build| {
        products::CheckedCompilation::from_settled_execution(
            products::CheckedCompilationInputs {
                settled: checked.execution().clone(),
                source_inspection: std::sync::Arc::clone(&inspection),
                build: std::sync::Arc::new(build),
                build_admission: checked.build_admission().clone(),
                provider_defaults: checked.provider_defaults().to_vec().into(),
            },
            std::sync::Arc::new(checked.sources().clone()),
            Default::default(),
            std::sync::Arc::clone(&publication),
        )
        .unwrap()
    };
    let mut missing = checked.build().clone();
    missing.accepted_build = None;
    let missing = rewrap(missing);
    let refusal = project_checked_package_policy(&missing, target, package).unwrap_err();
    assert!(diagnostics::join_messages(&refusal).contains("selected Build's accepted judgment"));
    let mut swapped = checked.build().clone();
    swapped.accepted_build = Some(other_judgment);
    let swapped = rewrap(swapped);
    let refusal = project_checked_package_policy(&swapped, target, package).unwrap_err();
    assert!(diagnostics::join_messages(&refusal).contains("Build's own invocation judgment"));
    assert_eq!(
        reviewed.semantic_bindings,
        checked.sources().accepted_semantic_bindings()
    );
    let bytes = reviewed.policy.canonical_bytes().unwrap();
    let recovered = PackagePolicyBaseline::recover_canonical(&bytes, Default::default()).unwrap();
    assert_eq!(recovered, reviewed.policy);
    assert_eq!(reviewed.policy_canonical_bytes, bytes.len());
    assert!(matches!(
        project_package_review(&checked, package, target, bytes.len() - 1),
        Err(PackageReviewProjectionError::PolicyBudget)
    ));
    assert!(project_package_review(&checked, package, target, bytes.len()).is_ok());
    let other = semantic_vocabulary::PackageKeyIdentity::from_digest([99; 32]).unwrap();
    assert!(project_checked_package_policy(&checked, target, other).is_err());
    let wrong_target = if target == target::TargetProfile::WindowsX64 {
        target::TargetProfile::UefiX64
    } else {
        target::TargetProfile::WindowsX64
    };
    assert!(project_checked_package_policy(&checked, wrong_target, package).is_err());
    session.close().unwrap();

    // An unused private local claim is still part of the reviewed subject.
    // Loaded but unrelated core claims must not erase this distinction.
    let claimed_source = format!(
        "{SOURCE}\nboundary machine unproved(value: u64) -> u64 ensures result == value;\n"
    );
    let (_workspace, mut session, claimed) =
        captured_review_compilation(&claimed_source, REVIEW_BUILD_SOURCE);
    let refusal = match claimed {
        Ok(_) => panic!("the ordinary checker must refuse the unchecked bodyless claim"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics::join_messages(&refusal)
            .contains("machine supply mode other than a checked body"),
        "{refusal:?}"
    );
    session.close().unwrap();

    // Another genuine Build judgment has the right phase but a different
    // supplying source; it cannot describe this package's execution.
    let foreign_source = format!("{REVIEW_BUILD_SOURCE}\n// Another retained Build source.\n");
    let (_workspace, mut session, other) = captured_review_compilation(SOURCE, &foreign_source);
    let other = other.unwrap_or_else(|diagnostics| panic!("valid foreign Build: {diagnostics:?}"));
    let mut foreign = checked.build().clone();
    foreign.accepted_build = other.build().accepted_build.clone();
    let foreign = rewrap(foreign);
    let refusal = project_checked_package_policy(&foreign, target, package).unwrap_err();
    assert!(
        diagnostics::join_messages(&refusal)
            .contains("retained declaration is absent or changed in the admitted source context"),
        "{refusal:?}"
    );
    session.close().unwrap();
}

#[test]
fn accepted_package_review_keeps_public_contracts_without_reopening_source() {
    use review_evidence::record::PackagePolicyBaseline;
    use review_evidence::{PackageReviewInput, project_package_review};
    // Public helpers must retain their guarantees even when the entry never
    // calls them. Review borrows the complete accepted package, not its entry's
    // reachable implementation alone.
    const SOURCE: &str = r#"
public machine identity(value: u64) -> u64
ensures result == value
{ value }
public machine quiet() {}
public data Pair { changed: u64; untouched: u64; }
public machine update(marker: u64, value: &mut Pair) { value.changed = marker; }
machine main() {}
"#;
    let (workspace, mut session, checked) =
        captured_review_compilation(SOURCE, REVIEW_BUILD_SOURCE);
    let checked = checked.unwrap_or_else(|errors| panic!("public judgment: {errors:?}"));
    let package = checked.sources().package_identity().unwrap();
    let target = checked.execution().target_profile().unwrap();
    let input = PackageReviewInput::supplied(&checked, &checked).unwrap();
    assert!(std::ptr::eq(
        input.module().unwrap(),
        checked.terminal_publication().accepted().unwrap().module()
    ));
    std::fs::remove_file(workspace.path().join("project/main.omg")).unwrap();
    std::fs::remove_file(workspace.path().join("project/build.omg")).unwrap();
    let reviewed = project_package_review(&checked, package, target, usize::MAX)
        .unwrap_or_else(|error| panic!("public review: {error:?}"));
    let publics: Vec<_> = reviewed
        .policy
        .callables()
        .callables()
        .iter()
        .filter(|row| format!("{:?}", row.role()) == "Public")
        .collect();
    assert_eq!(publics.len(), 3);
    let identity = publics
        .iter()
        .find(|row| row.identity().path().ends_with("identity"))
        .unwrap();
    assert_eq!(format!("{:?}", identity.supply()), "CheckedBody");
    assert_eq!(identity.parameters().len(), 1);
    assert_eq!(identity.contracts().len(), 1);
    assert_eq!(format!("{:?}", identity.contracts()[0].kind()), "Ensures");
    assert_eq!(
        identity.checked_service_reach().realized(),
        Some([].as_slice())
    );
    assert_eq!(
        identity.declared_synchronous_invocations(),
        Some([].as_slice())
    );
    assert_eq!(identity.declared_may_suspend(), Some(false));
    assert_eq!(identity.declared_may_block(), Some(false));
    let update = publics
        .iter()
        .find(|row| row.identity().path().ends_with("update"))
        .unwrap();
    let frame = format!("{:?}", update.mutation());
    assert!(frame.contains("parameter1:1"), "{frame}");
    assert!(
        frame.contains("changed") && !frame.contains("untouched"),
        "{frame}"
    );
    let bytes = reviewed.policy.canonical_bytes().unwrap();
    assert_eq!(
        PackagePolicyBaseline::recover_canonical(&bytes, Default::default()).unwrap(),
        reviewed.policy
    );
    session.close().unwrap();

    let wrong = SOURCE.replace("{ value }", "{ 37u64 }");
    let (_workspace, mut session, checked) =
        captured_review_compilation(&wrong, REVIEW_BUILD_SOURCE);
    let errors = match checked {
        Ok(_) => panic!("a false public guarantee cannot reach review"),
        Err(errors) => errors,
    };
    let messages = diagnostics::join_messages(&errors);
    assert!(
        messages.contains("ensures") || messages.contains("guarantee"),
        "{messages}"
    );
    session.close().unwrap();
}
