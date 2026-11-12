//! Real Build execution must carry restrictions into selected product admission.
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;
#[path = "support/joined_product_execution.rs"]
mod joined_product_execution;

use compiler::{CheckedCompileRequest, CompilationSession, CompileRequest};

fn prepare(relative: &str) -> fixture_package_inputs::ReviewedFixtureCompilation {
    fixture_package_inputs::reviewed_repository_fixture_package_inputs(
        &fixture_package_inputs::repo_root().join(relative),
        Some("macos_arm64"),
    )
    .unwrap_or_else(|diagnostics| panic!("fixture admission for {relative}: {diagnostics:?}"))
}

#[test]
fn calling_policies_share_product_code_across_targets_and_single_target_outputs() {
    use artifacts::compile_timings::counters;
    let mut prepared = prepare("tests/omega/pass/build/terminal_calling_policy_execution/main.omg");
    let profiles = ["macos_arm64", "linux_arm64"]
        .map(|name| target::TargetProfile::from_omega_target_name(Some(name)).unwrap());
    // One source input set admits both targets. Review Linux's exact entry as
    // well: a macOS-only acceptance must not authorize a different target.
    let linux = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
        &fixture_package_inputs::repo_root()
            .join("tests/omega/pass/build/terminal_calling_policy_execution/main.omg"),
        Some("linux_arm64"),
    )
    .expect("review Linux entry");
    let inputs = prepared
        .package_inputs
        .take()
        .expect("packaged policy fixture");
    let mut bindings = inputs
        .accepted_semantic_bindings()
        .cloned()
        .collect::<Vec<_>>();
    for binding in linux
        .package_inputs
        .as_ref()
        .expect("Linux package inputs")
        .accepted_semantic_bindings()
    {
        if !bindings.contains(binding) {
            bindings.push(binding.clone());
        }
    }
    prepared.package_inputs = Some(
        inputs
            .with_accepted_semantic_bindings(bindings)
            .expect("both reviewed target bindings belong to these exact source packages"),
    );
    let output = fixture_package_inputs::repo_root().join("build/shared-policy-publication");
    let compile = |targets: &[target::TargetProfile]| {
        let mut request = CompileRequest::for_targets(
            prepared.root_path.clone(),
            output.clone(),
            targets.iter().copied().map(Some).collect(),
        )
        .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact);
        if let Some(inputs) = &prepared.package_inputs {
            request = request.with_package_inputs(inputs.clone());
        }
        orchestration::compile_sources(&mut CompilationSession::default(), request)
            .expect("Product request")
            .into_outcomes()
            .into_iter()
            .map(|outcome| {
                let profile = outcome.target_profile().unwrap();
                (profile, outcome.into_result())
            })
            .collect::<Vec<_>>()
    };
    let source_checks = counters::product_source_check_count();
    let publications = counters::terminal_production_count();
    let builds = counters::source_build_execution_count();
    let suppliers = counters::provider_defaults_execution_count();
    let reports = compile(&profiles)
        .into_iter()
        .map(|(profile, result)| (profile, result.expect("calling policy admits the target")))
        .collect::<Vec<_>>();
    assert_eq!(
        counters::product_source_check_count() - source_checks,
        1,
        "targets share one Product judgment"
    );
    assert_eq!(
        counters::terminal_production_count() - publications,
        1,
        "policy activation must not compile a second code catalog"
    );
    assert_eq!(
        counters::source_build_execution_count() - builds,
        1,
        "target selection must not replay Build effects"
    );
    assert_eq!(
        counters::provider_defaults_execution_count() - suppliers,
        0,
        "target realization needs no core provider-default activation"
    );
    assert_eq!(reports.len(), 2);
    let first = reports[0]
        .1
        .retained_native_artifact()
        .unwrap()
        .psi_artifact();
    let second = reports[1]
        .1
        .retained_native_artifact()
        .unwrap()
        .psi_artifact();
    assert!(
        std::ptr::eq(first, second),
        "targets borrow the same published artifact, not equal copies"
    );

    // The same authored Build has no Linux x86-64 entry. That target precedes
    // macOS in canonical catalog order, not just in this caller's request, so
    // its refusal must not abort the later supported target.
    let refused = target::TargetProfile::from_omega_target_name(Some("linux_x86_64")).unwrap();
    let source_checks = counters::product_source_check_count();
    let publications = counters::terminal_production_count();
    let builds = counters::source_build_execution_count();
    let suppliers = counters::provider_defaults_execution_count();
    let mut mixed = compile(&[refused, profiles[0]]).into_iter();
    assert_eq!(
        counters::product_source_check_count() - source_checks,
        1,
        "a refused target cannot restart the shared source judgment"
    );
    assert_eq!(
        counters::terminal_production_count() - publications,
        1,
        "the surviving target publishes code once"
    );
    assert_eq!(
        counters::source_build_execution_count() - builds,
        1,
        "a refused target cannot replay Build effects"
    );
    assert_eq!(
        counters::provider_defaults_execution_count() - suppliers,
        0,
        "a refused sibling target cannot introduce a core provider-default activation"
    );
    let (profile, failed) = mixed.next().expect("first target outcome");
    assert_eq!(profile, refused);
    let diagnostics = match failed {
        Ok(_) => panic!("the fixture has no authored Linux x86-64 root binding"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("selected target `linux_x86_64` has no bound required root slot")),
        "the refused target must reach its missing entry obligation: {diagnostics:?}"
    );
    let (profile, survived) = mixed.next().expect("later supported target outcome");
    assert_eq!(profile, profiles[0]);
    let survived = survived.expect("earlier target failure must not poison this target");
    assert!(mixed.next().is_none(), "one outcome per requested target");
    let (_, supported) = reports
        .iter()
        .find(|(profile, _)| *profile == profiles[0])
        .expect("exact supported target in the successful batch");
    assert_eq!(
        survived
            .retained_native_artifact()
            .unwrap()
            .image()
            .output()
            .bytes,
        supported
            .retained_native_artifact()
            .unwrap()
            .image()
            .output()
            .bytes,
        "a failed sibling must not change the supported target's bytes"
    );
    for (profile, report) in reports {
        let mut single = compile(&[profile]);
        assert_eq!(single.len(), 1);
        let (_, single) = single.pop().unwrap();
        let single = single.expect("single-target policy execution");
        let actual = report.retained_native_artifact().unwrap();
        let expected = single.retained_native_artifact().unwrap();
        assert_eq!(
            actual.psi_artifact(),
            expected.psi_artifact(),
            "shared and single-target code must be identical for {profile:?}"
        );
        assert_eq!(
            actual.image().output().bytes,
            expected.image().output().bytes,
            "native bytes must not depend on a sibling target: {profile:?}"
        );
        if Some(profile) == target::TargetProfile::host_if_supported() {
            joined_product_execution::execute_report(report, &output);
        }
    }
}

struct ReplayedCallingPolicies {
    code: std::sync::Arc<provider_planning::calling_policy_plans::CallingProgram>,
    target: target::TargetProfile,
    realizations: Vec<provider_planning::calling_policy_plans::BoundaryCallingPlanRealization>,
    native: Vec<calling_conventions::BoundaryCallingApplication>,
    plans: Vec<effects::provider_plan::ProviderPlan>,
}

fn replay_calling_review(
    relative: &str,
    physical_refusal: Option<&str>,
) -> ReplayedCallingPolicies {
    use std::sync::Arc;

    let (
        package_inputs,
        substituted,
        code,
        wrong_code,
        source_owner,
        wrong_owner,
        expected,
        target,
        plans,
    ) = {
        let mut fixture = prepare(relative);
        let package_inputs = fixture.package_inputs.clone();
        // Reuse the admitted dependency snapshots and completed Builds for this
        // check instead of starting another source preparation.
        let checked = fixture_package_inputs::check_fixture(
            &mut fixture,
            "macos_arm64",
            Default::default(),
        )
        .expect("source and original calling policies check");
        let execution = checked.execution();
        let source = checked.source_inspection().program();
        let target = execution.selected_target_profile.expect("fixture target");
        let substituted = provider_planning::calling_policy_plans::prepare_boundary_calling_plans(
            target,
            package_inputs.as_ref(),
            &checked.providers().calling_program,
        )
        .expect("retain request/review correspondence and canonical helper choices");
        let code = Arc::clone(&checked.providers().calling_program);
        let expected = checked
            .providers()
            .boundary_calling_plan_realizations
            .iter()
            .map(|application| {
                application
                    .native_application()
                    .expect("original application")
            })
            .collect::<Vec<_>>();
        let plans = execution
            .selected_provider_provenance
            .iter()
            .filter(|provider| {
                if physical_refusal.is_some() {
                    let module = code.artifact.semantic_module();
                    return provider.plan.rows.iter().any(|row| {
                        let requirement = provider.plan.identities.text(row.requirement_identity);
                        module.calling_abi_signatures.iter().any(|declaration| {
                            declaration.physical_owner.is_err()
                                && module.identities.text(declaration.requirement_identity)
                                    == Some(requirement)
                        })
                    });
                }
                !provider.plan.schema.calling_applications.is_empty()
                    && provider.plan.rows.iter().all(|row| {
                        let requirement = provider.plan.identities.text(row.requirement_identity);
                        provider
                            .plan
                            .schema
                            .declaration
                            .methods
                            .iter()
                            .any(|method| {
                                provider
                                    .plan
                                    .schema
                                    .identity_text(method.requirement_identity)
                                    == requirement
                                    && !provider
                                        .plan
                                        .schema
                                        .calling_for_requirement(method.requirement_identity)
                                        .is_empty()
                            })
                    })
            })
            .map(|provider| provider.plan.clone())
            .collect::<Vec<_>>();
        assert!(
            !plans.is_empty(),
            "the witness must retain its selected calling or refused-owner rows"
        );
        assert!(
            physical_refusal.is_some() || !expected.is_empty(),
            "witness must actually activate policies"
        );

        // Equal-looking trees in a different real allocation do not own the
        // admitted invocation. Keep the same artifact so only that join breaks.
        let other_source = Arc::new((**source).clone());
        let wrong_owner = Arc::downgrade(&other_source);
        let wrong_code = provider_planning::calling_policy_plans::CallingProgram {
            source_owner: wrong_owner.clone(),
            artifact: Arc::clone(&code.artifact),
            source_records: Arc::clone(&code.source_records),
            source_preparation: Arc::clone(&code.source_preparation),
            evaluator: code.evaluator,
        };
        (
            package_inputs,
            substituted,
            code,
            wrong_code,
            Arc::downgrade(source),
            wrong_owner,
            expected,
            target,
            plans,
        )
    };
    assert!(
        source_owner.upgrade().is_none(),
        "prepared activation must not retain checked source"
    );
    assert!(
        wrong_owner.upgrade().is_none(),
        "the substituted source owner also expired"
    );
    assert!(!std::sync::Weak::ptr_eq(&source_owner, &wrong_owner));
    let rejected = substituted
        .evaluate(&wrong_code)
        .expect_err("another owner cannot reuse admission");
    assert!(
        rejected.iter().any(|diagnostic| diagnostic
            .message
            .contains("different checked source owner")),
        "{rejected:?}"
    );

    let decoded_code = provider_planning::calling_policy_plans::CallingProgram {
        source_owner: code.source_owner.clone(),
        evaluator: code.evaluator,
        artifact: Arc::new(
            terminal_codec::CanonicalTerminalArtifact::from_bytes(&code.artifact.to_bytes())
                .expect("checked Calling facts survive canonical transport"),
        ),
        source_records: Arc::clone(&code.source_records),
        source_preparation: Arc::clone(&code.source_preparation),
    };
    if relative == "tests/omega/pass/build/terminal_calling_policy_execution/main.omg" {
        assert_live_calling_authority(&decoded_code.artifact);
    }
    let prepared = provider_planning::calling_policy_plans::prepare_boundary_calling_plans(
        target,
        package_inputs.as_ref(),
        &decoded_code,
    )
    .expect("prepare requests and helpers after Source drop and canonical transport");
    let realizations = prepared
        .evaluate(&decoded_code)
        .expect("activate and attach review after Source drop");
    let actual = realizations
        .iter()
        .map(|application| {
            application
                .native_application()
                .expect("source-free native application")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual, expected,
        "Source release cannot change the complete native applications"
    );
    ReplayedCallingPolicies {
        code,
        target,
        realizations,
        native: actual,
        plans,
    }
}

fn assert_calling_review_outlives_source(
    relative: &str,
    physical_refusal: Option<&str>,
) -> (
    Vec<provider_planning::calling_policy_plans::BoundaryCallingPlanRealization>,
    Vec<effects::provider_plan::ProviderPlan>,
) {
    use std::sync::Arc;
    let ReplayedCallingPolicies {
        code,
        target,
        realizations,
        native: actual,
        plans,
    } = replay_calling_review(relative, physical_refusal);
    let recovered = provider_planning::calling_policy_plans::CallingProgram {
        source_owner: std::sync::Weak::new(),
        artifact: Arc::new(
            terminal_codec::CanonicalTerminalArtifact::from_bytes(&code.artifact.to_bytes())
                .expect("ordinary published artifact ingress"),
        ),
        source_records: Arc::from([]),
        source_preparation: Arc::default(),
        evaluator: code.evaluator,
    };
    let mut retained = None;
    for program in [&*code, &recovered] {
        let components = provider_planning::calling_policy_plans::component_calling_applications(
            target,
            program,
            &plans.iter().collect::<Vec<_>>(),
            &realizations,
        );
        if let Some(reason) = physical_refusal {
            let diagnostics = components.expect_err("ordinary owner cannot acquire a physical ABI");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(reason)),
                "{diagnostics:?}"
            );
            continue;
        }
        let components = components.expect("canonical ABI census needs no Source companions");
        assert!(
            !components.is_empty(),
            "the witness must join actual selected ABI rows"
        );
        for application in &components {
            assert!(
                actual.contains(application),
                "the Source-free row must retain its exact evaluated application"
            );
        }
        match &retained {
            None => retained = Some(components),
            Some(original) => assert_eq!(
                &components, original,
                "artifact reload preserves exact ABI row association"
            ),
        }
    }
    (realizations, plans)
}

/// The real decoded policy keeps its package obligations, not a serialized
/// permission. A custom live predicate need not export a comparison scope.
fn assert_live_calling_authority(artifact: &terminal_codec::CanonicalTerminalArtifact) {
    use semantic_vocabulary::compiler_evaluation::{
        DeclarationCustody, DeclarationSelectionAuthority,
    };
    struct LiveAuthority(bool);
    impl DeclarationSelectionAuthority for LiveAuthority {
        fn allows_declaration_selection(
            &self,
            _: semantic_vocabulary::PackageKeyIdentity,
            _: semantic_vocabulary::PackageKeyIdentity,
        ) -> bool {
            self.0
        }
        fn package_label(&self, package: semantic_vocabulary::PackageKeyIdentity) -> String {
            format!("{package:?}")
        }
    }
    let module = artifact.semantic_module();
    let root = module
        .calling_policy_requests
        .iter()
        .flat_map(|request| request.instances.as_ref().expect("real policy instances"))
        .next()
        .expect("fixture must retain an actual policy")
        .policy_declaration
        .as_ref()
        .expect("policy has exact checked declaration");
    let custody = module
        .scalar_applications
        .proof_scope
        .callable_headers
        .iter()
        .find(|(_, header)| header.declaration == *root)
        .expect("policy has retained compiler facts")
        .1
        .compiler_evaluation
        .as_ref()
        .expect("policy floor facts")
        .custody;
    assert!(matches!(custody, DeclarationCustody::Package(_)));
    let admit = |authority: &LiveAuthority| {
        provider_planning::calling_policy_plans::admit_terminal_calling_policy(
            artifact,
            *root,
            target::TargetProfile::MacosArm64,
            &[],
            custody,
            Some(authority),
        )
    };
    let deny = LiveAuthority(false);
    assert!(deny.declaration_selection_scope().is_none());
    let refusal = admit(&deny)
        .err()
        .expect("decoded facts grant no permission");
    assert!(refusal.contains("direct dependency authority"), "{refusal}");
    let allow = LiveAuthority(true);
    assert!(allow.declaration_selection_scope().is_none());
    admit(&allow).expect("live custom authority works without a stored scope");
}

#[test]
fn opaque_policy_review_activates_after_source_drop() {
    assert_calling_review_outlives_source(
        "tests/omega/pass/build/terminal_calling_policy_execution/main.omg",
        None,
    );
    let (realizations, plans) = assert_calling_review_outlives_source(
        "tests/omega/pass/facts/generic_calling_policy/shared_parent_roots_exit/main.omg",
        None,
    );
    let mut competing_root_observed = false;
    for plan in &plans {
        let root = plan.schema.root_application_identity().unwrap();
        for attachment in &plan.schema.calling_applications {
            let selected = realizations
                .iter()
                .find(|realization| {
                    realization.calling_owner_application() == root
                        && realization.materialized_signature().abi() == &attachment.abi
                        && realization.commitment == attachment.plan.commitment
                })
                .expect("the selected attachment has an actual evaluated application");
            competing_root_observed |= realizations.iter().any(|other| {
                other.calling_owner_application() != root
                    && other.materialized_signature().abi() == &attachment.abi
                    && other.commitment != selected.commitment
                    && other.native_application().unwrap() != selected.native_application().unwrap()
            });
        }
    }
    assert!(
        competing_root_observed,
        "the unused root must actually evaluate a different plan for the selected member ABI"
    );
    assert_calling_review_outlives_source(
        "tests/omega/pass/facts/generic_calling_policy/ordinary_parent_fused_exit/main.omg",
        Some("requirement owner `ParentEntry` is not an exact boundary trait"),
    );
}

#[test]
fn private_callback_policy_review_activates_after_source_drop() {
    assert_calling_review_outlives_source(
        "tests/omega/pass/build/terminal_private_callback_calling/main.omg",
        None,
    );
}

#[test]
fn inherited_callback_policy_retains_selected_child_after_source_drop() {
    // A callback's inbound plan is selected by its registrar occurrence;
    // it is not a component export selected by a provider row.
    let replayed = replay_calling_review(
        "tests/omega/pass/layouts/private_callback_inherited_policy_exit/main.omg",
        None,
    );
    let realizations = replayed.realizations;
    assert!(
        realizations.iter().any(|selected| {
            realizations.iter().any(|sibling| {
                selected.calling_owner_application() != sibling.calling_owner_application()
                    && selected.materialized_signature().abi()
                        == sibling.materialized_signature().abi()
                    && selected.boundary_entry_plan != sibling.boundary_entry_plan
                    && selected
                        .native_application()
                        .unwrap()
                        .calling_owner_application
                        != sibling
                            .native_application()
                            .unwrap()
                            .calling_owner_application
            })
        }),
        "both child policies must actually evaluate, sharing one inherited ABI"
    );
}

#[test]
fn ranked_policy_admission_activates_after_source_drop() {
    assert_calling_review_outlives_source(
        "tests/omega/pass/facts/generic_calling_policy/ranked_helper_exit/main.omg",
        None,
    );
}

#[test]
fn late_application_attachment_outlives_source_and_preserves_recovered_attribution() {
    // Descriptions escape this scope without borrowing the checked program.
    let (program, recovered, valid, invalid, source_owner) = {
        let prepared = prepare("tests/omega/pass/build/retained_behavior_admission/main.omg");
        let mut request = CheckedCompileRequest::new(&prepared.root_path, Some("macos_arm64"));
        request.package_inputs = prepared.package_inputs;
        let checked = orchestration::check_source(&mut CompilationSession::default(), request)
            .result
            .expect("source checks");
        let source = checked.source_inspection().program();
        let source_owner = std::sync::Arc::downgrade(source);
        let sources = checked.sources().exact_toolchain_sources();
        let signature = checked
            .execution()
            .selected_program_entry
            .as_ref()
            .expect("selected entry")
            .source_signature();
        let describe = |declaration, name| {
            psi_compiler::TerminalApplicationCustody::prepare(
                terminal_psi::PreparedNominalRoot::new(declaration, name, None)
                    .map_err(|reason| vec![diagnostics::Diagnostic::error(reason.to_string())]),
                None,
                Some(psi_compiler::TerminalCompositionRoots::default()),
            )
        };
        let valid = describe(signature.declaration(), signature.machine_name());
        let invalid = describe("", "missing_entry");
        let produced = psi_compiler::TerminalProductionRequest::new(
            source,
            psi_compiler::TerminalMachineSelection::Name("main"),
            sources,
        )
        .produce(Default::default(), &mut psi_compiler::UnmeasuredProduction)
        .expect("publish without applications");
        // Logical package imports retain the complete module cohort. Give this
        // real library companion a finite budget larger than a small unit record.
        let limits = compiler_storage::RecordLimits {
            elements: 64 * 1024 * 1024,
            ..Default::default()
        };
        let encoded = compiler_storage::encode(&produced, limits).expect("encode source companion");
        let recovered: psi_compiler::ProducedTerminalProgram =
            compiler_storage::decode(&encoded, limits).expect("recover source companion");
        (
            produced.into_shared_portable().unwrap(),
            recovered.into_shared_portable().unwrap(),
            valid,
            invalid,
            source_owner,
        )
    };
    assert!(
        source_owner.upgrade().is_none(),
        "published code must not retain checked source"
    );
    let mut original_labels = None;
    for published in [&program, &recovered] {
        let identity = published.artifact().view().identity();
        assert!(
            published
                .attach_application(&invalid, &mut psi_compiler::UnmeasuredProduction)
                .is_err()
        );
        let application = published
            .attach_application(&valid, &mut psi_compiler::UnmeasuredProduction)
            .expect("late valid application");
        let [root] = <[_; 1]>::try_from(application.roots).expect("one exact root");
        let root = root.expect("root retained");
        assert!(root.provenance.label(root.machine).contains("main"));
        let helper_labels = published
            .artifact()
            .semantic_module()
            .machines
            .iter()
            .filter(|machine| machine.id != root.machine)
            .map(|machine| root.provenance.label(machine.id))
            .collect::<Vec<_>>();
        assert!(
            helper_labels
                .iter()
                .any(|label| label.starts_with("machine `") && label.contains("check")),
            "a non-entry source label must survive without borrowing or reconstructing it from the entry: {helper_labels:?}"
        );
        match &original_labels {
            None => original_labels = Some(helper_labels),
            Some(original) => assert_eq!(
                &helper_labels, original,
                "recovery must preserve the shared diagnostic companion"
            ),
        }
        assert_eq!(
            published.artifact().view().identity(),
            identity,
            "attachment cannot replace code"
        );
    }
    assert_eq!(
        program.artifact().semantic_module(),
        recovered.artifact().semantic_module()
    );
}

#[test]
fn executed_values_keep_helper_identity_and_omit_untaken_exclusions() {
    let prepared = prepare("tests/omega/pass/build/executed_exclusion_values_exit/main.omg");
    let mut request = CheckedCompileRequest::new(&prepared.root_path, Some("macos_arm64"));
    request.package_inputs = prepared.package_inputs.clone();
    let checked = orchestration::check_source(&mut CompilationSession::default(), request)
        .result
        .expect("executed Build checks");
    let grants = &checked.build().config.grants;
    assert_eq!(
        grants.len(),
        1,
        "the untaken trust acceptance grants nothing"
    );
    assert_eq!(grants[0].selector, "AcceptedService");
    assert_eq!(
        checked
            .source_inspection()
            .program()
            .symbols
            .name(grants[0].selecting_machine),
        "configure",
        "acceptance retains its helper rather than attributing it to build"
    );
    let rows = &checked.build().config.behavior_exclusions;
    exclusion_tickets_reject_foreign_receivers_and_replay(&checked);
    assert_eq!(
        rows.len(),
        4,
        "two authored Abort sites, physical class and service"
    );
    assert!(rows.iter().all(|row| {
        checked
            .source_inspection()
            .program()
            .typed
            .symbols
            .name(row.selecting_machine)
            == "configure"
    }));
    assert_eq!(
        rows.iter()
            .filter(|row| matches!(
                row.kind,
                effects::AuthoredBehaviorExclusionKind::CrashCause {
                    cause: terminal_psi::CrashCause::Abort,
                    ..
                }
            ))
            .count(),
        2
    );
    assert!(rows.iter().any(|row| matches!(
        row.kind,
        effects::AuthoredBehaviorExclusionKind::PhysicalAuthorityClass {
            class: effects::TerminalAuthorityClass::ProcessInput,
            ..
        }
    )));
    assert!(rows.iter().any(|row| matches!(&row.kind, effects::AuthoredBehaviorExclusionKind::Service { name, .. } if name == "UnusedService")));
    let host = target::TargetProfile::host_if_supported();
    let windows =
        Some(target::TargetProfile::from_omega_target_name(Some("windows_x86_64")).unwrap());
    let foreign = if host == windows {
        Some(target::TargetProfile::from_omega_target_name(Some("macos_arm64")).unwrap())
    } else {
        windows
    };
    let targets = vec![foreign, host];
    let output = fixture_package_inputs::repo_root().join("build/executed-exclusions-native");
    let mut request = CompileRequest::for_targets(prepared.root_path, output.clone(), targets)
        .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact);
    if let Some(inputs) = prepared.package_inputs {
        request = request.with_package_inputs(inputs);
    }
    let outcomes = orchestration::compile_sources(&mut CompilationSession::default(), request)
        .expect("target batch")
        .into_outcomes();
    for outcome in outcomes {
        let host = outcome.target_profile() == target::TargetProfile::host_if_supported();
        let report = outcome
            .into_result()
            .expect("only executed exclusions constrain each target");
        if host {
            joined_product_execution::execute_report(report, &output);
        }
    }
}

fn exclusion_tickets_reject_foreign_receivers_and_replay(checked: &compiler::CheckedCompilation) {
    use psi_compiler::TerminalBuildBindingOperation as Operation;
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
    use terminal_interpreter::{
        TerminalEffect, TerminalEffectHandler, TerminalEffectResult, TerminalScalarValue,
        TerminalStructuralValue,
    };
    let produced = psi_compiler::TerminalProductionRequest::new(
        checked.source_inspection().program(),
        psi_compiler::TerminalMachineSelection::Name("build"),
        checked.sources().exact_toolchain_sources(),
    )
    .produce(Default::default(), &mut psi_compiler::UnmeasuredProduction)
    .expect("source-issued Build");
    let artifact = produced.view();
    let program = terminal_interpreter::TerminalProgram::borrow(artifact);
    let original = TerminalStructuralValue::minted(
        1,
        program.entry().unwrap().structural_parameters[0].structural_type,
        Vec::new(),
    );
    let host = build_settings::host::bindings::TerminalBuildBindingHost::new(
        artifact,
        produced.build_bindings(),
        original.clone(),
        None,
    )
    .unwrap();
    let calls = produced.build_bindings().calls_for(artifact).unwrap();
    let commit = calls
        .iter()
        .find(|call| call.action == Operation::Exclude)
        .unwrap();
    let preparation = calls
        .iter()
        .find(|call| call.action == Operation::Prepare && call.request == commit.request)
        .unwrap();
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    };
    let invoke =
        |call: &psi_compiler::TerminalBuildBindingCall, arguments, structural_arguments| {
            let declaration = artifact
                .module()
                .boundary_machines
                .iter()
                .find(|declaration| declaration.id == call.boundary)
                .unwrap();
            host.adapter(declaration.into())
                .handle_effect_result(&TerminalEffect::BoundaryCall {
                    operation: call.operation,
                    boundary: call.boundary,
                    arguments,
                    structural_arguments,
                    byte_sequence_arguments: Vec::new(),
                    completion_receipts: Vec::new(),
                    result: declaration.result.clone(),
                })
        };
    let mut foreign = original.clone();
    foreign.opaque_identity = 2;
    let error = invoke(preparation, Vec::new(), vec![foreign]).unwrap_err();
    assert!(format!("{error:?}").contains("original compiler-issued Build"));
    let error = invoke(commit, vec![integer(0), integer(1)], Vec::new()).unwrap_err();
    assert!(format!("{error:?}").contains("consumed or belongs to another request"));
    let TerminalEffectResult::Scalar(ticket) =
        invoke(preparation, Vec::new(), vec![original]).unwrap()
    else {
        panic!("preparation must issue a ticket");
    };
    let other = calls
        .iter()
        .find(|call| call.action == Operation::Exclude && call.request != commit.request)
        .unwrap();
    let error = invoke(other, vec![ticket, integer(1)], Vec::new()).unwrap_err();
    assert!(format!("{error:?}").contains("consumed or belongs to another request"));
    let error = invoke(commit, vec![ticket, integer(u128::MAX)], Vec::new()).unwrap_err();
    assert!(format!("{error:?}").contains("outside its vocabulary"));
    assert_eq!(
        invoke(commit, vec![ticket, integer(1)], Vec::new()).unwrap(),
        TerminalEffectResult::Unit
    );
    let error = invoke(commit, vec![ticket, integer(1)], Vec::new()).unwrap_err();
    assert!(format!("{error:?}").contains("consumed or belongs to another request"));
    assert_eq!(host.into_executed().unwrap().len(), 1);
}

#[test]
fn executed_crash_selection_rejects_only_the_trapping_target() {
    let prepared =
        prepare("tests/omega/fail/targets/distinct_roots_exclusion_selected_body/main.omg");
    let targets = ["windows_x86_64", "macos_arm64"]
        .map(|name| Some(target::TargetProfile::from_omega_target_name(Some(name)).unwrap()))
        .to_vec();
    let mut request = CompileRequest::for_targets(
        prepared.root_path,
        std::env::temp_dir().join("omega-exclusion-targets"),
        targets,
    )
    .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact);
    if let Some(inputs) = prepared.package_inputs {
        request = request.with_package_inputs(inputs);
    }
    let outcomes = orchestration::compile_sources(&mut CompilationSession::default(), request)
        .expect("target batch")
        .into_outcomes();
    assert_eq!(
        outcomes.len(),
        2,
        "an admission refusal cannot drop its sibling"
    );
    assert_eq!(
        outcomes
            .iter()
            .map(|outcome| outcome.target_profile().unwrap().target_name())
            .collect::<Vec<_>>(),
        // CompileRequest validation admits targets in TargetProfile::ALL order.
        vec!["macos_arm64", "windows_x86_64"],
        "application results must keep the admitted target order"
    );
    for outcome in outcomes {
        let target = outcome.target_profile().unwrap().target_name();
        match outcome.into_result() {
            Ok(_) => assert_eq!(target, "windows_x86_64"),
            Err(diagnostics) => {
                assert_eq!(target, "macos_arm64", "{diagnostics:?}");
                assert!(
                    diagnostics.iter().any(|diagnostic| diagnostic
                        .message
                        .contains("behavior exclusion violated: crash cause Trap")),
                    "{diagnostics:?}"
                );
            }
        }
    }
}

#[test]
fn service_selections_use_exact_product_identity_and_actual_reach() {
    for (fixture, rejected) in [
        ("pass/build/runtime_static_service_exclusion_exit", false),
        ("pass/build/sibling_module_service_exclusion_exit", false),
        ("fail/build/static_service_exclusion_invoked", true),
        ("fail/build/static_service_exclusion_relayed", true),
    ] {
        let prepared = prepare(&format!("tests/omega/{fixture}/main.omg"));
        let output = fixture_package_inputs::repo_root().join("build/excluded-services-native");
        let mut request = CompileRequest::for_targets(
            prepared.root_path,
            output.clone(),
            vec![target::TargetProfile::host_if_supported()],
        )
        .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact);
        if let Some(inputs) = prepared.package_inputs {
            request = request.with_package_inputs(inputs);
        }
        let outcomes = orchestration::compile_sources(&mut CompilationSession::default(), request)
            .expect("service target batch")
            .into_outcomes();
        let result = outcomes
            .into_iter()
            .next()
            .expect("one service product")
            .into_result();
        match result {
            Ok(report) => {
                assert!(!rejected, "{fixture} must refuse its reached service");
                joined_product_execution::execute_report(report, &output);
            }
            Err(diagnostics) => {
                assert!(rejected, "{fixture}: {diagnostics:?}");
                assert!(
                    diagnostics.iter().any(|diagnostic| diagnostic
                        .message
                        .contains("behavior exclusion violated: service")),
                    "{fixture}: {diagnostics:?}"
                );
            }
        }
    }
}

#[test]
fn physical_selections_use_the_shared_mechanism_classification() {
    for (fixture, target, reason) in [
        (
            "asm_halt_excluded_machine_control",
            "linux_x86_64",
            "excluded physical authority class MachineControl",
        ),
        (
            "asm_runtime_register_excluded_port_io",
            "linux_x86_64",
            "excluded physical authority class PortIo",
        ),
        (
            "asm_system_register_excluded_machine_control",
            "linux_arm64",
            "excluded physical authority class MachineControl",
        ),
    ] {
        let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
            &fixture_package_inputs::repo_root()
                .join(format!("tests/omega/fail/inline_asm/{fixture}/main.omg")),
            Some(target),
        )
        .expect("physical fixture admission");
        let profile = target::TargetProfile::from_omega_target_name(Some(target)).unwrap();
        let mut request = CompileRequest::for_targets(
            prepared.root_path,
            std::env::temp_dir().join("omega-excluded-physical"),
            vec![Some(profile)],
        )
        .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact);
        if let Some(inputs) = prepared.package_inputs {
            request = request.with_package_inputs(inputs);
        }
        let outcomes = orchestration::compile_sources(&mut CompilationSession::default(), request)
            .expect("physical target batch")
            .into_outcomes();
        let diagnostics = match outcomes.into_iter().next().unwrap().into_result() {
            Ok(_) => panic!("{fixture} must reject its physical mechanism"),
            Err(diagnostics) => diagnostics,
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(reason)),
            "{fixture}: {diagnostics:?}"
        );
    }
}
