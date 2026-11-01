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
    .expect("fixture admission")
}

#[test]
fn calling_policies_share_product_code_across_targets_and_single_target_outputs() {
    use artifacts::compile_timings::counters;
    let prepared = prepare("tests/omega/pass/build/terminal_calling_policy_execution/main.omg");
    let profiles = ["macos_arm64", "linux_arm64"]
        .map(|name| target::TargetProfile::from_omega_target_name(Some(name)).unwrap());
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
        omega_build::compile_sources(&mut CompilationSession::default(), request)
            .expect("Product request")
            .into_outcomes()
            .into_iter()
            .map(|outcome| {
                let profile = outcome.target_profile().unwrap();
                (
                    profile,
                    outcome
                        .into_result()
                        .expect("calling policy admits the target"),
                )
            })
            .collect::<Vec<_>>()
    };
    let source_checks = counters::settled_check_totals().0;
    let publications = counters::terminal_production_count();
    let reports = compile(&profiles);
    assert_eq!(
        counters::settled_check_totals().0 - source_checks,
        1,
        "targets share one Product judgment"
    );
    assert_eq!(
        counters::terminal_production_count() - publications,
        1,
        "policy activation must not compile a second code catalog"
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
    for (profile, report) in reports {
        let mut single = compile(&[profile]);
        assert_eq!(single.len(), 1);
        let (_, single) = single.pop().unwrap();
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

#[test]
fn late_application_attachment_outlives_source_and_preserves_recovered_attribution() {
    // Descriptions escape this scope without borrowing the checked program.
    let (program, recovered, valid, invalid, source_owner) = {
        let prepared = prepare("tests/omega/pass/build/retained_behavior_admission/main.omg");
        let mut request = CheckedCompileRequest::new(&prepared.root_path, Some("macos_arm64"));
        request.package_inputs = prepared.package_inputs;
        let checked = omega_build::check_source(&mut CompilationSession::default(), request)
            .result
            .expect("source checks");
        let source = checked.execution().provider_source.program();
        let source_owner = std::sync::Arc::downgrade(source);
        let sources = checked.sources().exact_toolchain_sources();
        let describe = |name| {
            psi_compiler::TerminalApplicationCustody::prepare(
                source,
                psi_compiler::TerminalMachineSelection::Name(name),
                None,
                Some(psi_compiler::TerminalCompositionRoots::default()),
                sources,
                &mut psi_compiler::UnmeasuredProduction,
            )
        };
        let valid = describe("main");
        let invalid = describe("missing_entry");
        let produced = psi_compiler::TerminalProductionRequest::new(
            source,
            psi_compiler::TerminalMachineSelection::Name("main"),
            sources,
        )
        .produce(Default::default(), &mut psi_compiler::UnmeasuredProduction)
        .expect("publish without applications");
        let encoded = compiler_storage::encode(&produced, Default::default())
            .expect("encode source companion");
        let recovered: psi_compiler::ProducedTerminalProgram =
            compiler_storage::decode(&encoded, Default::default())
                .expect("recover source companion");
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
    let checked = omega_build::check_source(&mut CompilationSession::default(), request)
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
            .execution()
            .provider_source
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
            .execution()
            .provider_source
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
    let outcomes = omega_build::compile_sources(&mut CompilationSession::default(), request)
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
        checked.execution().provider_source.program(),
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
    let host = build_evaluation::terminal_bindings::TerminalBuildBindingHost::new(
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
    let error = invoke(other, vec![ticket.clone(), integer(1)], Vec::new()).unwrap_err();
    assert!(format!("{error:?}").contains("consumed or belongs to another request"));
    let error = invoke(commit, vec![ticket.clone(), integer(u128::MAX)], Vec::new()).unwrap_err();
    assert!(format!("{error:?}").contains("outside its vocabulary"));
    assert_eq!(
        invoke(commit, vec![ticket.clone(), integer(1)], Vec::new()).unwrap(),
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
    let outcomes = omega_build::compile_sources(&mut CompilationSession::default(), request)
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
                assert_eq!(target, "macos_arm64");
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
        let outcomes = omega_build::compile_sources(&mut CompilationSession::default(), request)
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
        let outcomes = omega_build::compile_sources(&mut CompilationSession::default(), request)
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
