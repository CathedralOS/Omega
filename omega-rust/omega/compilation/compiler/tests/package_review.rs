//! Review publishes source-free evidence under the caller's exact byte budget.

#[path = "support/build_sequence.rs"]
mod build_sequence;

mod external_provider_reload;
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[path = "support/native_entry_controls.rs"]
mod native_entry_controls;
mod target_constant_custody;

use compiler::{CheckedCompileRequest, CompilationSession, PackageReviewFailure};
use package_evidence::PackageReviewProjectionError;

#[test]
fn reusable_package_checks_declarations_without_deploying_unused_float_bodies() {
    // These libraries have no program entry. Their UEFI target request belongs
    // to this package check; the positive also covers ordinary source checking.
    let target = "uefi_x86_64";
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
        let source = fixture_package_inputs::repo_root()
            .join("tests/omega")
            .join(fixture)
            .join("main.omg");
        let result = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
            &source,
            Some(target),
        )
        .and_then(|mut prepared| {
            fixture_package_inputs::check_fixture(&mut prepared, target, Default::default())
        });
        match (result, refusal) {
            (Ok(_), None) => {}
            (Err(diagnostics), Some(fragment)) => assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(fragment)),
                "{fixture}: expected {fragment:?}, got {diagnostics:?}"
            ),
            (Ok(_), Some(fragment)) => panic!("{fixture}: accepted instead of {fragment:?}"),
            (Err(diagnostics), None) => panic!("{fixture}: {diagnostics:?}"),
        }
    }
}

#[test]
fn callback_review_rejoins_candidate_types_reports_and_target_members() {
    use package_evidence::{PackageReviewInput, project_checked_package_policy};
    let target = "macos_arm64";
    let profile = target::TargetProfile::from_canonical_target_name(target).unwrap();
    let fixture = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/build/terminal_private_callback_calling/main.omg");
    let mut prepared =
        fixture_package_inputs::reviewed_repository_fixture_package_inputs(&fixture, Some(target))
            .expect("private callback fixture admission");
    let checked = fixture_package_inputs::check_fixture(&mut prepared, target, Default::default())
        .expect("checked private callback fixture");
    let package = checked.sources().package_identity().expect("root package");
    project_checked_package_policy(&checked, profile, package).expect("original callback review");
    let indirect = checked
        .providers()
        .boundary_calling_plan_realizations
        .iter()
        .find(|row| {
            row.materialized_signature()
                .owner_requirement_identity()
                .contains("IndirectRegistrar")
        })
        .expect("indirect registrar's source-authored declaration ABI")
        .materialized_signature();
    assert!(indirect.callback_layout_catalog().is_empty());
    assert!(
        indirect.callback_demands().is_empty(),
        "a stored pointer grants no direct private field destination"
    );
    let parameter = indirect.parameters()[0];
    assert_eq!(
        indirect.shapes()[usize::from(parameter)].class,
        calling_conventions::BoundaryValueClass::Reference
    );
    let signature = checked
        .providers()
        .boundary_calling_plan_realizations
        .iter()
        .map(|row| row.materialized_signature())
        .find(|signature| signature.callback_layout_catalog().len() == 2)
        .expect("registrar retains both private inline paths");
    let entries = signature.callback_layout_catalog();
    let fixed = entries
        .iter()
        .find(|entry| {
            let source = entry.review_source();
            let data = checked
                .typed
                .data_definition_by_symbol(source.root.declaration)
                .unwrap();
            checked
                .typed
                .data_field(data, source.fields.last().unwrap().declaration)
                .unwrap()
                .name
                .as_str()
                == "first"
        })
        .expect("fixed-layout callback path");
    let variable = entries
        .iter()
        .find(|entry| {
            entry.review_source().fields.last().unwrap().declaration
                != fixed.review_source().fields.last().unwrap().declaration
        })
        .expect("target-member callback path");
    let fixed_source = fixed.review_source();
    let variable_source = variable.review_source();
    let fixed_field = fixed_source.fields.last().unwrap();
    let variable_field = variable_source.fields.last().unwrap();
    assert_ne!(
        fixed_field.child.storage_identity,
        variable_field.child.storage_identity
    );
    assert_eq!(fixed.terminal_slot().size, variable.terminal_slot().size);
    let reject = |typed: typed_trees::TypedTrees, fragment: &str| {
        let candidate = checked_trees::CheckedTrees::with_roots(typed, checked.facts.clone());
        let errors = project_checked_package_policy(
            PackageReviewInput::supplied(&candidate, &checked),
            profile,
            package,
        )
        .expect_err("changed candidate must not retain the original callback review");
        assert!(
            errors.iter().any(|error| error.message.contains(fragment)),
            "expected {fragment:?}, got {errors:?}"
        );
    };
    let mut typed = checked.typed.clone().into_inner();
    let data = typed
        .data_definition_by_symbol(fixed_source.root.declaration)
        .unwrap();
    let alternative = typed
        .data_field(data, variable_field.declaration)
        .unwrap()
        .type_reference;
    let member = typed.data_members(data).iter().find(|member| matches!(member, typed_trees::data::DataMember::Field(field) if field.symbol == fixed_field.declaration)).unwrap();
    let handle = typed.data_members.handle_of(member).unwrap();
    let typed_trees::data::DataMember::Field(field) = typed.data_members.get_mut(handle) else {
        panic!("record field")
    };
    field.type_reference = alternative;
    reject(
        typed,
        "callback inline field selects another exact child application",
    );

    let mut typed = checked.typed.clone().into_inner();
    let coordinate = fixed_field.child.layout.as_ref().unwrap();
    let geometry = match &mut typed.plan_laid_layouts[coordinate.ordinal as usize].realizations {
        typed_trees::PlanLaidRealizations::Invariant(geometry) => geometry,
        typed_trees::PlanLaidRealizations::TargetInputs(_) => {
            &mut typed
                .plan_laid_target_layouts
                .get_mut(fixed_field.child.realization.as_ref().unwrap().member)
                .geometry
        }
    };
    geometry.private_callback_demands[0].placements =
        vec![layout_plans::LayoutPlacementReport::At { offset: 0 }];
    reject(
        typed,
        "callback source layout changed its complete selected report",
    );

    let mut typed = checked.typed.clone().into_inner();
    let coordinate = variable_field.child.realization.as_ref().unwrap();
    typed
        .plan_laid_target_layouts
        .get_mut(coordinate.member)
        .target_facts
        .pointer_size = 4;
    reject(
        typed,
        "callback source realization changed its exact target facts",
    );
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
fn review_retains_exact_same_geometry_phantom_local_applications() {
    use package_evidence::encoding::PackagePolicyRecoveryLimits;
    use package_evidence::record::PackagePolicyBaseline;

    let fixture = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/providers/phantom_local_calling_applications/main.omg");
    let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
        &fixture,
        Some("macos_arm64"),
    )
    .expect("phantom application fixture inputs");
    let inputs = prepared
        .package_inputs
        .expect("package-aware phantom fixture");
    let package = inputs.root();
    let mut session = CompilationSession::default();
    let build = execute_build(
        &mut session,
        CheckedCompileRequest {
            package_inputs: Some(inputs),
            ..CheckedCompileRequest::new(&prepared.root_path, Some("macos_arm64"))
        },
    );
    let checked = session
        .check_package_build(build)
        .expect("checked phantom applications");
    let native = checked
        .providers()
        .calling_applications(&[])
        .expect("complete native catalog");
    for realization in &checked.providers().boundary_calling_plan_realizations {
        assert!(
            native.contains(&realization.native_application().unwrap()),
            "deduplication must retain every distinct complete policy application"
        );
    }
    let mut repeated = checked.providers().clone();
    repeated.boundary_calling_plan_realizations.extend(
        checked
            .providers()
            .boundary_calling_plan_realizations
            .iter()
            .cloned(),
    );
    assert_eq!(
        native,
        repeated.calling_applications(&[]).unwrap(),
        "repeating complete applications cannot create ambiguous catalog entries"
    );
    let captured = package_evidence::project_checked_package_policy(
        &checked,
        target::TargetProfile::MacosArm64,
        package,
    )
    .expect("capture every exact local application");
    let policy = &captured;
    let provider = policy
        .selected_providers()
        .plans()
        .iter()
        .find(|plan| plan.provider_type() == "ProbeMap")
        .expect("selected ProbeMap review");
    let method = provider
        .methods()
        .iter()
        .find(|method| method.name() == "inspect")
        .expect("inspect review");
    let applications = method.calling();
    assert_eq!(
        applications.len(),
        2,
        "review must not drop or collapse either local application"
    );
    for application in applications {
        assert_eq!(
            application.abi().operands.len(),
            1,
            "callback actuals are not declaration ABI operands"
        );
        assert_eq!(application.abi().operands[0].ordinal, 0);
    }
    let first = &applications[0];
    let second = &applications[1];
    assert_eq!(
        first.abi().requirement_identity,
        second.abi().requirement_identity
    );
    assert_eq!(
        first.abi().declaring_application,
        second.abi().declaring_application
    );
    assert_eq!(first.shape_graph(), second.shape_graph());
    assert_eq!(first.physical(), second.physical());
    assert_ne!(first.abi(), second.abi());
    assert_ne!(first.abi().id(), second.abi().id());
    assert_ne!(
        first.canonical_bytes().unwrap(),
        second.canonical_bytes().unwrap()
    );
    let bytes = policy.canonical_bytes().unwrap();
    let recovered =
        PackagePolicyBaseline::recover_canonical(&bytes, PackagePolicyRecoveryLimits::default())
            .unwrap();
    assert_eq!(bytes, recovered.canonical_bytes().unwrap());
    let recovered_method = recovered
        .selected_providers()
        .plans()
        .iter()
        .find(|plan| plan.provider_type() == "ProbeMap")
        .unwrap()
        .methods()
        .iter()
        .find(|method| method.name() == "inspect")
        .unwrap();
    assert_eq!(applications, recovered_method.calling());
}

#[test]
fn review_retains_installation_reach_across_staggered_dependency_resolution() {
    let target = "macos_arm64";
    let fixture = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/providers/staggered_installation_reach_exit/main.omg");
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
    let selected = checked.execution().plans();
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
    session
        .review_package_build(build, usize::MAX)
        .expect("exact installed reach remains reviewable");
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
    assert!(
        first
            .source
            .facts()
            .consumed_units
            .as_ref()
            .is_some_and(|units| !units.is_empty())
    );
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
        );
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

#[test]
fn historical_constant_closure_requires_exact_target_and_provider() {
    use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct};
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
        orchestration::compile_sources(&mut compiler::CompilationSession::default(), request)
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
    let fixture = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/traits/runtime_dyn_slot_metadata_exit/main.omg");
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
    let recovered = package_evidence::record::PackagePolicyBaseline::recover_canonical(
        &bytes,
        package_evidence::encoding::PackagePolicyRecoveryLimits::default(),
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
    fixture: &str,
    target: &str,
) -> package_evidence::record::PackagePolicyBaseline {
    use package_evidence::encoding::PackagePolicyRecoveryLimits;
    use package_evidence::record::PackagePolicyBaseline;

    let fixture = fixture_package_inputs::repo_root().join(fixture);
    let prepared =
        fixture_package_inputs::reviewed_repository_fixture_package_inputs(&fixture, Some(target))
            .expect("fixture inputs");
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
        "tests/omega/pass/facts/static_callback_reach_boundary_schema/main.omg",
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
    use package_evidence::record::PackageReviewNominalOwner;

    // `value + 1.0` closes an application of the toolchain `Float::add`
    // overload, whose requirement identity typed Psi qualifies with the
    // toolchain owner namespace.
    let recovered = reviewed_and_recovered(
        "tests/omega/pass/float/live_float_parameter_across_call_exit/main.omg",
        "linux_x86_64",
    );
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
}

#[test]
fn opaque_realization_publishes_the_ceiling_it_inherits_from_its_requirement() {
    // `read` repeats neither `reaches Probe` nor `blocks`; the requirement it
    // satisfies publishes both, and an opaque supply has no body from which to
    // publish anything else.
    let recovered = reviewed_and_recovered(
        "tests/omega/pass/boundary/native_provider_unoffered_read/main.omg",
        "linux_x86_64",
    );
    let blocking_reads = recovered
        .callables()
        .callables()
        .iter()
        .filter(|callable| {
            callable.checked_may_block() && callable.identity().path().contains("read")
        })
        .collect::<Vec<_>>();
    assert!(
        !blocking_reads.is_empty(),
        "the fixture realizes the blocking Probe::read opaquely"
    );
    for leaf in blocking_reads {
        assert_eq!(
            leaf.declared_may_block(),
            Some(true),
            "{}",
            leaf.identity().path()
        );
    }
}

#[test]
fn completed_scalar_call_preserves_wrapping_index_guard() {
    let target = "linux_x86_64";
    let mut failures = Vec::new();
    for (fixture, accepted) in [
        (
            "pass/facts/compound_index_guard_through_call_result_exit",
            true,
        ),
        (
            "fail/facts/compound_index_guard_through_call_result_rejected",
            false,
        ),
        (
            "pass/facts/compound_index_guard_through_match_join_exit",
            true,
        ),
        (
            "fail/facts/compound_index_guard_through_match_join_rejected",
            false,
        ),
        ("fail/facts/compound_index_guard_wrong_call_actual", false),
    ] {
        let source = fixture_package_inputs::repo_root()
            .join("tests/omega")
            .join(fixture)
            .join("main.omg");
        let result = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
            &source,
            Some(target),
        )
        .and_then(|mut prepared| {
            fixture_package_inputs::check_fixture(&mut prepared, target, Default::default())
        });
        match result {
            Ok(_) if accepted => eprintln!("SOURCE_CHECK {fixture}: accepted"),
            Err(diagnostics)
                if !accepted
                    && diagnostics.iter().any(|diagnostic| {
                        diagnostic.message.contains("cannot prove index")
                            && diagnostic.message.contains("within length 200")
                    }) =>
            {
                eprintln!("SOURCE_CHECK {fixture}: intended bound refusal {diagnostics:?}")
            }
            Ok(_) => failures.push(format!("{fixture}: invalid upperless source accepted")),
            Err(diagnostics) => failures.push(format!("{fixture}: {diagnostics:?}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// The foreign contract keeps its authored method identity, while its closed
// outer and local arguments control exactly which call instances it can supply.
#[test]
fn native_binding_keeps_applied_boundary_scopes_through_package_reload() {
    check_native_binding_applications(false);
}

#[test]
fn native_binding_keeps_distinct_local_bindings_in_one_boundary_plan() {
    check_native_binding_applications(true);
}

fn check_native_binding_applications(select_second_local: bool) {
    use checked_trees::provider_declarations::{
        ProviderApplicationView, ProviderDeclarationOrigin, ProviderDeclarationRequest,
        ProviderDeclarationRow,
    };
    let root = fixture_package_inputs::repo_root();
    let fixture =
        root.join("tests/omega/fail/calls/foreign_applied_callback_requires_selected_entry_plan");
    let scratch = root.join("build").join(format!(
        "omega-applied-native-binding-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&scratch).unwrap();
    let original = std::fs::read_to_string(fixture.join("main.omg")).unwrap();
    let (declarations, _) = original
        .split_once("machine main()")
        .expect("fixture entry");
    let callback = &declarations[declarations.find("data CallbackBody").unwrap()..];
    let mut source = format!(
        "{declarations}\n{}\npublic boundary trait Getter<Value> {{ machine read() -> i32 reaches; }}\nmachine main() {{}}\n",
        callback.replace("CallbackBody", "SpareCallbackBody")
    );
    if select_second_local {
        source.push_str(&callback.replace("CallbackBody", "DormantCallbackBody"));
    }
    std::fs::write(scratch.join("main.omg"), source).unwrap();
    let build = std::fs::read_to_string(fixture.join("build.omg")).unwrap();
    let start = build
        .find("    Build::bind<Registrar<u64>::install>")
        .expect("exact applied binding");
    let end = start + build[start..].find("    });").unwrap() + "    });".len();
    let binding = &build[start..end];
    let second = binding.replace("Registrar<u64>", "Registrar<u32>");
    let closed = binding.replace(
        "Registrar<u64>::install",
        "Registrar<u16>::install<CallbackBody::notify>",
    );
    let getter = binding
        .replace("Registrar<u64>::install", "Getter<u8>::read")
        .replace("atexit", "getpid");
    let spare = closed
        .replace("CallbackBody", "SpareCallbackBody")
        .replace("libc.so.6", "/lib/x86_64-linux-gnu/libc.so.6");
    let dormant = closed.replace(
        "CallbackBody",
        if select_second_local {
            "DormantCallbackBody"
        } else {
            "SpareCallbackBody"
        },
    );
    let mut build = build;
    build.insert_str(
        end,
        &format!(
            "\n{second}\n{closed}\n{getter}{}",
            if select_second_local {
                format!("\n{spare}")
            } else {
                String::new()
            }
        ),
    );
    build.push_str(&format!(
        "\nmachine unused(builder: &mut Build) {{\n{dormant}\n}}\n"
    ));
    std::fs::write(scratch.join("build.omg"), build).unwrap();
    let mut prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
        &scratch.join("main.omg"),
        Some("linux_x86_64"),
    )
    .expect("closed native applications form without executing callbacks");
    let checked =
        fixture_package_inputs::check_fixture(&mut prepared, "linux_x86_64", Default::default())
            .expect("declared and selected native applications check");
    let package = checked.sources().package_identity().unwrap();
    let native = checked
        .providers()
        .native_build_plans
        .iter()
        .filter(|candidate| candidate.plan.origin_package_identity == Some(package))
        .collect::<Vec<_>>();
    assert_eq!(
        native.len(),
        4,
        "all selected locals remain inside their one outer boundary plan"
    );
    let mut applications = native
        .iter()
        .map(|candidate| {
            candidate
                .plan
                .identities
                .text(candidate.plan.provider_application)
        })
        .collect::<Vec<_>>();
    applications.sort_unstable();
    applications.dedup();
    assert_eq!(
        applications.len(),
        4,
        "no static application may merge into another"
    );
    let mut aliases = Vec::new();
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    for candidate in &native {
        assert!(
            candidate
                .plan
                .validate_candidate_against_schema()
                .is_empty()
        );
        let rows = candidate
            .plan
            .rows
            .iter()
            .zip(&candidate.provenance.row_realizations)
            .map(|(row, realization)| {
                let origin = realization.native().expect("actual native row");
                let application = checked
                    .typed
                    .product_operand_applications
                    .get(origin.application.expect("prepared native application"));
                let scope = application.scope.scope(&checked.typed).unwrap();
                let outer = scope
                    .portable_application_identity(&checked.typed)
                    .and_then(|identity| checked.typed.type_identity_text(identity))
                    .expect("exact portable outer application");
                assert_eq!(
                    candidate.plan.schema.root_application_identity(),
                    Some(outer.as_str()),
                    "native schema must retain its checked boundary application; selected application: {}; receivers: {:?}",
                    candidate.plan.identities.text(candidate.plan.provider_application),
                    candidate.plan.schema.declaration.receiver_applications,
                );
                ProviderDeclarationRow {
                    requirement: origin.requirement,
                    origin: ProviderDeclarationOrigin::BuildNative {
                        requirement: origin.requirement,
                        application: origin.application,
                    },
                    requirement_identity: candidate.plan.identities.text(row.requirement_identity),
                    declared_requirement_identity: candidate.plan.identities.text(row.declared_requirement_identity),
                    application: ProviderApplicationView::External,
                }
            })
            .collect::<Vec<_>>();
        let declarations = checked.provider_declarations(ProviderDeclarationRequest {
            rows: &rows,
            named_application: None,
            provider_application_identity: Some(
                candidate
                    .plan
                    .identities
                    .text(candidate.plan.provider_application),
            ),
            installation_reach: false,
        });
        for (row, declaration) in candidate.plan.rows.iter().zip(declarations.rows) {
            for application in declaration
                .external_applications
                .applications
                .expect("authenticated native application aliases")
            {
                aliases.push(effects::SelectedExternalRequirementApplication {
                    provider_plan_report_identity: candidate.plan.report_fingerprint(),
                    provider_plan_digest: candidate.plan.identity_digest(),
                    selected_requirement_identity: identities
                        .intern(candidate.plan.identities.text(row.requirement_identity)),
                    requirement_identity: identities.intern(&application),
                });
            }
        }
    }
    let facts = effects::SelectedProviderPlanFacts::from_selected_plans(
        native
            .iter()
            .map(|candidate| candidate.plan.clone())
            .collect(),
    )
    .unwrap()
    .with_external_requirement_applications(aliases, &identities.build())
    .unwrap();
    let closed = checked
        .typed
        .closed_requirement_applications()
        .filter(|application| {
            checked
                .typed
                .trait_machine_signature(application.template)
                .is_some_and(|(owner, _)| checked.symbols.name(owner.symbol) == "Registrar")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        closed.len(),
        if select_second_local { 3 } else { 2 },
        "selected and dormant callback instances are both formed"
    );
    let selected_closed = native
        .iter()
        .flat_map(|candidate| &candidate.provenance.row_realizations)
        .filter_map(|realization| realization.native())
        .filter_map(|origin| origin.application)
        .map(|application| {
            checked
                .typed
                .product_operand_applications
                .get(application)
                .declaration
        })
        .filter(|selected| {
            closed
                .iter()
                .any(|application| application.signature.symbol == *selected)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        selected_closed.len(),
        if select_second_local { 2 } else { 1 },
        "only executed Registrar binding intents are selected"
    );
    for application in closed {
        let (owner, _) = checked
            .typed
            .trait_machine_signature(application.template)
            .unwrap();
        let identity = checked
            .typed
            .trait_requirement_callable_identity_text(owner, application.signature);
        assert_eq!(
            facts.selected_requirement_row(&identity).is_some(),
            selected_closed.contains(&application.signature.symbol),
            "selected origin must supply its own local instance and refuse the dormant sibling: {identity}"
        );
        if let Some((plan, row)) = facts.selected_requirement_row(&identity) {
            let origin = native
                .iter()
                .find(|candidate| candidate.plan == *plan)
                .unwrap()
                .provenance
                .row_realizations
                .iter()
                .filter_map(|realization| realization.native())
                .find(|origin| {
                    origin.application.is_some_and(|handle| {
                        checked
                            .typed
                            .product_operand_applications
                            .get(handle)
                            .declaration
                            == application.signature.symbol
                    })
                })
                .expect("selected row retains its own checked application");
            assert_eq!(
                row.binding,
                origin.origin.primary.evaluated.provider_binding(),
                "each exact local must keep its own locator"
            );
            assert_eq!(plan.identities.text(row.requirement_identity), identity);
            assert_ne!(row.requirement_identity, row.declared_requirement_identity);
            assert!(row.calling_signature.is_some());
        }
    }
    let handoff = checked
        .package_build_handoff()
        .expect("capture checked applications");
    let bytes = package_compilation::native_output_codec::encode(&handoff);
    let restored = package_compilation::native_output_codec::decode(&bytes, handoff.clone())
        .expect("reload exact application graphs");
    assert_eq!(
        package_compilation::native_output_codec::digest(&restored),
        package_compilation::native_output_codec::digest(&handoff)
    );
    assert_eq!(
        (
            restored.native_declarations().len(),
            restored.native_intents().len()
        ),
        if select_second_local { (6, 5) } else { (5, 4) }
    );
    let mut typed = checked.typed.clone().into_inner();
    let selections = build_time_evaluation::native_bindings::collect_captured(
        &mut typed,
        restored.native_declarations(),
        restored.native_intents(),
    )
    .expect("authenticate receiving applications");
    assert_eq!(selections.len(), if select_second_local { 5 } else { 4 });
    let mut outer = selections
        .iter()
        .map(|selection| {
            typed
                .product_operand_application_identity(selection.application.unwrap())
                .unwrap()
                .unwrap()
        })
        .collect::<Vec<_>>();
    outer.sort_unstable();
    outer.dedup();
    assert_eq!(
        outer.len(),
        4,
        "different method locals share their outer boundary"
    );
    let mut expected_receiving = native
        .iter()
        .flat_map(|candidate| {
            candidate
                .plan
                .rows
                .iter()
                .zip(&candidate.provenance.row_realizations)
                .map(move |(row, realization)| {
                    let origin = realization
                        .native()
                        .expect("actual selected native binding");
                    let effects::provider_plan::EvaluatedNativeBinding::Import(import) =
                        &origin.origin.primary.evaluated
                    else {
                        panic!("this fixture retains actual libc imports");
                    };
                    (
                        candidate
                            .plan
                            .schema
                            .root_application_identity()
                            .unwrap()
                            .to_owned(),
                        candidate
                            .plan
                            .identities
                            .text(row.requirement_identity)
                            .to_owned(),
                        import.locator().identity_digest().as_bytes().to_owned(),
                    )
                })
        })
        .collect::<Vec<_>>();
    let mut actual_receiving = selections
        .iter()
        .map(|selection| {
            let outer = typed
                .product_operand_application_identity(selection.application.unwrap())
                .unwrap()
                .unwrap();
            let outer = typed
                .type_identity_text(outer)
                .expect("portable receiving outer scope");
            let closed = typed.closed_requirement_application(selection.requirement);
            let (owner, declared) = typed
                .trait_machine_signature(
                    closed
                        .as_ref()
                        .map_or(selection.requirement, |closed| closed.template),
                )
                .unwrap();
            let selected = typed.trait_requirement_callable_identity_text(
                owner,
                closed.as_ref().map_or(declared, |closed| closed.signature),
            );
            let semantic_vocabulary::NativeBindingLocator::Import(locator) = &selection.locator
            else {
                panic!("this fixture reloads actual libc imports");
            };
            let locator =
                target::normalize_foreign_locator(locator.clone(), target::TargetProfile::LinuxX64)
                    .unwrap();
            (
                outer,
                selected,
                locator.identity_digest().as_bytes().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    expected_receiving.sort();
    actual_receiving.sort();
    assert_eq!(
        actual_receiving, expected_receiving,
        "package reload must preserve every exact outer/method/locator association"
    );
    for missing in [false, true] {
        let mut declarations = restored.native_declarations().to_vec();
        if missing {
            declarations[0].application = None;
        } else {
            declarations[0]
                .application
                .as_mut()
                .unwrap()
                .identity
                .push_str("changed");
        }
        let errors = build_time_evaluation::native_bindings::collect_captured(
            &mut checked.typed.clone().into_inner(),
            &declarations,
            restored.native_intents(),
        )
        .expect_err("a saved binding cannot erase or substitute its application");
        let expected = if missing {
            "closed static boundary application"
        } else {
            "complete outer application identity"
        };
        assert!(
            errors.iter().any(|error| error.message.contains(expected)),
            "{errors:?}"
        );
    }
    std::fs::remove_dir_all(scratch).unwrap();
}

// Restrict an existing working registrar to the callback it actually receives.
// Its program, C provider, layout and exit oracle remain the original control.
#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn closed_native_binding_reloads_and_executes_the_descriptor_callback() {
    use std::sync::Arc;
    let root = fixture_package_inputs::repo_root();
    let fixture = root.join("tests/omega/pass/layouts/private_callback_inherited_policy_exit");
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
        compiler::CompilationProduct::Native(compilation_products::NativeProductRequest {
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
        terminal_psi_to_abstract_operations::recover_native_derivation_inputs(artifact, bytes)
            .expect("reload the closed binding from its canonical artifact and saved inputs");
    assert_eq!(recovered, pending.terminal);
    assert_eq!(recovered_permissions, pending.accepted_package_permissions);
    assert!(installation_assumptions.is_empty());
    let module = recovered.artifact().semantic_module();
    assert!(
        !module.callback_applications.is_empty(),
        "the existing native callback must actually be emitted"
    );
    let selected = &recovered
        .native_realization_proposal()
        .unwrap()
        .providers()
        .selection;
    for callback in &module.callback_applications {
        let operation = module
            .operations()
            .find(|operation| operation.id == callback.registrar)
            .expect("actual emitted registrar operation");
        let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = operation.kind else {
            panic!("registrar must retain its exact boundary call");
        };
        let declaration = module
            .boundary_machines
            .iter()
            .find(|declaration| declaration.id == boundary)
            .unwrap();
        let requirement = module.identities.text(declaration.identity).unwrap();
        let (plan, row) = selected
            .selected_requirement_row(requirement)
            .expect("the emitted closed call must select its own native row");
        assert_ne!(
            plan.identities.text(row.requirement_identity),
            plan.identities.text(row.declared_requirement_identity)
        );
        let abi = module
            .calling_abi_identity(boundary)
            .expect("actual emitted boundary ABI");
        assert_eq!(row.calling_signature, Some(abi.id()));
        let wrong = module
            .calling_abi_signatures
            .iter()
            .find(|declaration| declaration.id != abi.id())
            .expect("the callback's distinct ABI supplies an actual substitution control");
        let mut changed = plan.clone();
        let index = plan
            .rows
            .iter()
            .position(|candidate| std::ptr::eq(candidate, row))
            .unwrap();
        changed.rows[index].calling_signature = Some(wrong.id);
        assert_ne!(
            changed.identity_digest(),
            plan.identity_digest(),
            "a different real ABI must not reuse the selected binding commitment"
        );
    }
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
        .expect("complete recovered native work through ordinary package reconciliation")
        .into_report()
        .expect("complete actual native lowering after saved-input recovery");
    drop(prepared);
    native_entry_controls::inspect(&report, &scratch)
        .expect("preserve the original native entry readback falsifiers");
    let published = compilation_products::publication::publish_compilation(
        report,
        &scratch.join("native-output"),
    )
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
