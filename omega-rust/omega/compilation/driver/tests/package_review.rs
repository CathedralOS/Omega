//! Review publishes source-free evidence under the caller's exact byte budget.

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::{CheckedCompileRequest, CompilationSession, PackageReviewFailure};
use package_evidence::PackageReviewProjectionError;

fn execute_build(
    session: &mut CompilationSession,
    request: CheckedCompileRequest,
) -> Result<compiler::PackageBuild, compiler::CompileFailure> {
    let mut invocation = session.begin_package_build(request)?;
    let returned = invocation.execution().map(|execution| {
        let mut host = execution.host().expect("Build host");
        let mut activation = host.start().expect("Build activation");
        while !activation.advance(1024).expect("Build step").returned {}
        activation.complete().expect("normal Build return")
    });
    session.complete_package_build(invocation, returned)
}

#[test]
fn policy_layout_queries_preserve_input_values_and_branching() {
    let target = "linux_x86_64";
    let fixture = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/layouts/policy_layout_query_values/main.omg");
    let mut prepared =
        fixture_package_inputs::reviewed_repository_fixture_package_inputs(&fixture, Some(target))
            .expect("policy query fixture has admitted package inputs");
    let inputs = prepared.package_inputs.clone().expect("root custody");
    for entry in [
        "main.omg",
        "skipped.omg",
        "computed_extent.omg",
        "target_geometry.omg",
        "guarded_contexts.omg",
    ] {
        let request = CheckedCompileRequest {
            package_inputs: Some(inputs.clone()),
            ..CheckedCompileRequest::new(&prepared.root_path.with_file_name(entry), Some(target))
        };
        fixture_package_inputs::check_fixture_request(&mut prepared, request)
            .expect("policy input queries settle before formation without executing skipped arms");
    }
    let request = CheckedCompileRequest {
        package_inputs: Some(inputs.clone()),
        ..CheckedCompileRequest::new(
            &prepared.root_path.with_file_name("invalid_extent.omg"),
            Some(target),
        )
    };
    let refused = fixture_package_inputs::check_fixture_request(&mut prepared, request)
        .expect_err("the actual query extent must contain every policy field");
    let rendered = refused
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("field `value` ends at bit 96, past the fixed size 32 bits"),
        "{rendered}"
    );

    let request = CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(
            &prepared
                .root_path
                .with_file_name("invalid_guarded_contract.omg"),
            Some(target),
        )
    };
    let refused = fixture_package_inputs::check_fixture_request(&mut prepared, request)
        .expect_err("identical output geometry cannot erase another context's policy precondition");
    let rendered = refused
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("call require_extent from Policy::plan: 4 > 4"),
        "{rendered}"
    );
}

#[test]
fn dependency_layouts_precede_formation_and_borrow_checking() {
    let target = "linux_x86_64";
    let fixture = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/build/dependency_layout_cohorts/main.omg");
    let mut prepared =
        fixture_package_inputs::reviewed_repository_fixture_package_inputs(&fixture, Some(target))
            .expect("layout dependency fixture has admitted package inputs");
    let inputs = prepared.package_inputs.clone().expect("root custody");
    for entry in ["main.omg", "valid_disjoint.omg"] {
        let request = CheckedCompileRequest {
            package_inputs: Some(inputs.clone()),
            ..CheckedCompileRequest::new(&prepared.root_path.with_file_name(entry), Some(target))
        };
        fixture_package_inputs::check_fixture_request(&mut prepared, request)
            .expect("completed geometry preserves valid physical views");
    }
    for (entry, fragments) in [
        (
            "invalid_bounds.omg",
            &["recast target needs 12 bytes", "region holds 4"][..],
        ),
        (
            "invalid_overlap.omg",
            &["borrow `second`", "borrow `first` is still active"][..],
        ),
    ] {
        let request = CheckedCompileRequest {
            package_inputs: Some(inputs.clone()),
            ..CheckedCompileRequest::new(&prepared.root_path.with_file_name(entry), Some(target))
        };
        let diagnostics = fixture_package_inputs::check_fixture_request(&mut prepared, request)
            .expect_err("layout queries must not hide formation or exclusive-loan obligations");
        let rendered = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        for fragment in fragments {
            assert!(rendered.contains(fragment), "{entry}: {rendered}");
        }
    }
}

#[test]
fn dependency_frontends_retain_constant_values_without_admitting_new_callers() {
    let target = "linux_x86_64";
    let fixture = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/build/reviewed_dependency_constants/main.omg");
    let prepared =
        fixture_package_inputs::reviewed_repository_fixture_package_inputs(&fixture, Some(target))
            .expect("constant dependency fixture has admitted package inputs");
    let inputs = prepared.package_inputs.clone().expect("root custody");
    let retained_source_count = || {
        phase_profile::snapshot()
            .iter()
            .find(|row| row.name == "source_activation.retained_sources")
            .map_or(0, |row| row.count)
    };
    for complete_dependency_review in [false, true] {
        let request = CheckedCompileRequest {
            package_inputs: Some(inputs.clone()),
            ..CheckedCompileRequest::new(&prepared.root_path, Some(target))
        };
        let occurrences = fixture_package_inputs::fixture_occurrences(&prepared, request)
            .expect("dependency-first fixture occurrences");
        assert_eq!(occurrences.len(), 2, "one dependency and its consumer");

        // Exercise both source owners: an ordinary Build leaves Product work
        // pending, while review completes it before the consumer is prepared.
        let mut session = CompilationSession::default();
        let root_package = inputs.root();
        let mut dependency_bundles = Vec::new();
        for mut occurrence in occurrences {
            let is_dependency = occurrence
                .package_inputs
                .as_ref()
                .expect("occurrence custody")
                .root()
                != root_package;
            if is_dependency && !complete_dependency_review {
                // This source owns an invalid Product body outside the consumer's
                // selected imports. A dependency Build must leave it pending; the
                // consumer must complete math's bodies, not this excluded body.
                occurrence.root_path = occurrence.root_path.with_file_name("invalid_product.omg");
            }
            occurrence.package_inputs = Some(
                occurrence
                    .package_inputs
                    .take()
                    .expect("occurrence custody")
                    .with_complete_dependency_generated_sources(dependency_bundles.clone())
                    .expect("complete dependency Build outputs, including empty bundles"),
            );
            let retained_before = retained_source_count();
            let build = execute_build(&mut session, occurrence)
                .expect("prepare dependency or consumer Build");
            // A profiled, single-test replay pins that the semantic witnesses
            // exercised reuse, rather than succeeding through fresh checking.
            // Counters are process-global; ordinary unprofiled runs still pin
            // acceptance and refusal without making a performance assertion.
            if phase_profile::enabled() && !is_dependency {
                assert!(
                    retained_source_count() > retained_before,
                    "consumer did not activate dependency source:\n{}",
                    phase_profile::render(),
                );
            }
            if build.package_inputs().root() != root_package {
                dependency_bundles.push(build.generated_source_bundle().clone());
                if !complete_dependency_review {
                    continue;
                }
            }
            session
                .review_package_build(build, usize::MAX)
                .expect("review retained constant dependency and consumer");
        }

        // Retained values cannot discharge a new caller's false precondition,
        // and selecting an old excluded module must check its pending bodies.
        for (entry, fragments) in [
            ("invalid_bounds.omg", &["lower <= upper", "4 <= 3"][..]),
            (
                "invalid_membership.omg",
                &["terminal expression expects `u64`, got `bool`"][..],
            ),
        ] {
            let invalid = prepared.root_path.with_file_name(entry);
            let refused = execute_build(
                &mut session,
                CheckedCompileRequest {
                    package_inputs: Some(
                        inputs
                            .clone()
                            .with_complete_dependency_generated_sources(dependency_bundles.clone())
                            .expect("invalid caller uses the same dependency Build outputs"),
                    ),
                    ..CheckedCompileRequest::new(&invalid, Some(target))
                },
            )
            .map_err(PackageReviewFailure::Compilation)
            .and_then(|build| session.review_package_build(build, usize::MAX));
            let diagnostics = match refused {
                Err(PackageReviewFailure::Compilation(failure)) => failure.into_diagnostics(),
                Err(failure) => {
                    panic!("{entry} reached an unrelated review failure: {failure:?}")
                }
                Ok(_) => panic!("retained dependency source admitted {entry}"),
            };
            let rendered = diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            for fragment in fragments {
                assert!(rendered.contains(fragment), "{entry}: {rendered}");
            }
        }
    }
}

#[test]
fn review_product_keeps_exact_custody_and_policy_budget() {
    let target = "linux_x86_64";
    let fixture = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/calls/machine_self_call_recursion_compile/main.omg");
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
        )
        .expect("complete package Build");
        session.review_package_build(build, maximum_bytes)
    };
    let first = review(usize::MAX).expect("review product");
    let bytes = first.evidence.policy_canonical_bytes;
    assert_eq!(first.evidence.policy.package(), package);
    assert_eq!(first.evidence.generated_source_bundle.package(), package);
    assert_eq!(
        first.evidence.source_consumption_commitment,
        first
            .evidence
            .generated_source_bundle
            .source_consumption_commitment()
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
    assert!(
        first
            .source
            .facts()
            .consumed_units
            .as_ref()
            .is_some_and(|units| !units.is_empty())
    );
    assert!(!first.source.facts().tolerated_unsettled_service_fields);
    assert!(bytes > 0);
    assert!(matches!(
        review(bytes - 1),
        Err(PackageReviewFailure::Projection(
            PackageReviewProjectionError::PolicyBudget
        ))
    ));
    let exact = review(bytes).expect("exact budget is sufficient");
    assert_eq!(exact.evidence.policy, first.evidence.policy);
    assert_eq!(
        exact.evidence.source_consumption_commitment,
        first.evidence.source_consumption_commitment
    );
}

#[test]
fn review_policy_round_trip_preserves_authored_and_inferred_reach() {
    use package_evidence::encoding::PackagePolicyRecoveryLimits;
    use package_evidence::record::PackagePolicyBaseline;

    let root = fixture_package_inputs::repo_root();
    for (fixture, expected) in [
        (
            "tests/omega/pass/facts/unannotated_imported_boundary_call_through_local_exit/support/main.omg",
            "inferred",
        ),
        (
            "tests/omega/pass/effects/optional_reach_declared_slack_exit/main.omg",
            "slack",
        ),
        (
            "tests/omega/pass/effects/optional_reach_empty_exit/main.omg",
            "empty",
        ),
    ] {
        let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
            &root.join(fixture),
            Some("macos_arm64"),
        )
        .expect("reviewable reach fixture");
        let mut session = CompilationSession::default();
        let build = execute_build(
            &mut session,
            CheckedCompileRequest {
                package_inputs: prepared.package_inputs,
                ..CheckedCompileRequest::new(&prepared.root_path, Some("macos_arm64"))
            },
        )
        .expect("complete package Build");
        let reviewed = session
            .review_package_build(build, usize::MAX)
            .expect("reach review");
        let bytes = reviewed
            .evidence
            .policy
            .canonical_bytes()
            .expect("encode reach policy");
        let recovered = PackagePolicyBaseline::recover_canonical(
            &bytes,
            PackagePolicyRecoveryLimits::default(),
        )
        .expect("recover source-free reach policy");
        // Provider identity tables retain construction interning beyond the
        // rows they serialize. Compare persisted meaning, then inspect the
        // recovered reach fields themselves below.
        assert_eq!(
            recovered.canonical_bytes().expect("re-encode policy"),
            bytes
        );
        let callables = recovered.callables().callables().iter().collect::<Vec<_>>();
        assert!(!callables.is_empty(), "{fixture}");
        match expected {
            "inferred" => {
                let callable = callables
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
            }
            "slack" => {
                let callable = callables
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
                assert_eq!(callable.published_service_reach(), Some(declared));
            }
            "empty" => {
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
            _ => unreachable!(),
        }
    }
}

/// Source-produced closure custody is an SDK boundary, not an example command.
/// Equal layout values cannot substitute target or provider identity.
#[test]
fn historical_constant_closure_requires_exact_target_and_provider() {
    use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct, compile};
    use std::path::Path;

    fn produce(
        root: &Path,
        target_name: &str,
        product: RequestedCompileProduct,
    ) -> compiler::CompileReport {
        let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
            root,
            Some(target_name),
        )
        .expect("source-backed package inputs");
        let mut request = CompileRequest::new(CompileOptions {
            root_path: prepared.root_path,
            build_dir: Some(std::env::temp_dir().join(format!(
                "omega-layout-closure-{}-{target_name}",
                std::process::id(),
            ))),
            target_name: Some(target_name.to_owned()),
        })
        .with_requested_product(product);
        if let Some(inputs) = prepared.package_inputs {
            request = request.with_package_inputs(inputs);
        }
        compile(request)
            .expect("request admission")
            .into_single_report()
            .expect("source reaches its requested product")
    }

    let root = fixture_package_inputs::repo_root()
        .join("samples/cli/facts/target_layout_derived_constant/main.omg");
    let retained = produce(
        &root,
        "linux_x86_64",
        RequestedCompileProduct::TerminalArtifact,
    )
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

    let scratch = fixture_package_inputs::repo_root()
        .join("build")
        .join(format!(
            "omega-equal-layout-provider-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
    std::fs::create_dir(&scratch).expect("private source control directory");
    let source = std::fs::read_to_string(&root).expect("authored witness");
    let alternate_source = source.replace(
        "select_provider<Answer, Small>",
        "select_provider<Answer, Same>",
    );
    assert_ne!(alternate_source, source);
    std::fs::write(scratch.join("main.omg"), alternate_source).unwrap();
    std::fs::copy(root.with_file_name("build.omg"), scratch.join("build.omg")).unwrap();
    let alternate = produce(
        &scratch.join("main.omg"),
        "linux_x86_64",
        RequestedCompileProduct::TerminalArtifact,
    )
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
    std::fs::remove_dir_all(scratch).unwrap();
    // This emits a Windows product but executes only its Terminal semantics on
    // this host. The frozen source's branch traps unless the derived value is
    // exactly 9 (the Windows provider extent 8 plus 1), rather than Linux's 5.
    let windows = produce(
        &root,
        "windows_x86_64",
        RequestedCompileProduct::NativeArtifact,
    );
    let artifact = windows
        .retained_native_artifact()
        .expect("Windows native emission");
    let observation = terminal_runtime::execute(terminal_runtime::TerminalRuntimeRequest {
        native_provider: None,
        x86_flags: None,
        provider_proposal: None,
        artifact: artifact.psi_artifact(),
        target: None,
        provisions: windows.runtime_provisions(),
        fuel_allowance: 100_000,
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
