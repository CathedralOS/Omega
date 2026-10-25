//! Host readback of actual source-produced entry storage, including the Build
//! record. No second evaluator or synthesized Terminal machine supplies results.
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::{CheckedCompileRequest, CompilationSession};
use fixture_package_inputs::{repo_root, reviewed_repository_fixture_package_inputs};
use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, IntegerValue};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalEntryStorageError, TerminalEntryStorageValue, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalProgram, TerminalScalarValue, TerminalStorageContents,
    TerminalStructuralInputs, TerminalStructuralValue,
};

fn produce(
    machine: &str,
) -> (
    psi_compiler::ProducedTerminalArtifact,
    package_compilation::build_execution::build_value::BuildValueAdmission,
    build_time_evaluation::BuildTimeValue,
) {
    // The fixture harness restages one process-local path for each request.
    // Hold that source owner until compilation has finished reading it.
    static STAGING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _staging = STAGING.lock().unwrap();
    let root = repo_root().join("tests/omega/pass/build/receiver_view_writeback_exit/main.omg");
    let prepared =
        reviewed_repository_fixture_package_inputs(&root, None).expect("sealed fixture inputs");
    let mut request = CheckedCompileRequest::new(&prepared.root_path, None);
    request.package_inputs = prepared.package_inputs;
    let checked = CompilationSession::default()
        .check(request)
        .result
        .unwrap_or_else(|error| panic!("{machine}: {error:?}"));
    let typed = &checked.execution().program.typed;
    let admission =
        package_compilation::build_execution::build_value::BuildValueAdmission::admit(typed)
            .unwrap();
    let selected = typed
        .machines()
        .iter()
        .find(|entry| entry.name.as_str() == machine)
        .unwrap();
    let scope = package_compilation::build_execution::prepare_filesystem_scope(
        &prepared.root_path,
        None,
        &source::SourceMap::default(),
        None,
        None,
        None,
        None,
    )
    .unwrap();
    let initial = admission.initial_value(typed, selected, &scope);
    let artifact = psi_compiler::TerminalProductionRequest::new(
        &checked.execution().program,
        psi_compiler::TerminalMachineSelection::Name(machine),
        checked.sources().exact_toolchain_sources(),
    )
    .produce(Default::default(), &mut psi_compiler::UnmeasuredProduction)
    .unwrap_or_else(|error| panic!("{machine}: {error:?}"));
    (artifact, admission, initial)
}

fn field(artifact: &CanonicalTerminalArtifact, name: &str) -> Path {
    let program = TerminalProgram::borrow(artifact.view());
    let entry = program.entry().unwrap();
    let root = entry.structural_parameters[0].structural_type;
    let declaration = artifact
        .semantic_module()
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root)
        .unwrap();
    let terminal_psi::StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        panic!("record entry");
    };
    let identities = &artifact.semantic_module().identities;
    Path::Field(
        fields
            .iter()
            .find(|field| identities.text(field.identity) == Some(name))
            .unwrap()
            .id,
    )
}

#[test]
fn completed_helpers_return_original_entry_storage_to_the_embedding() {
    for (machine, identifier) in [
        ("configure", b"omega".as_slice()),
        ("configure_build_readback", b"com.example.omega".as_slice()),
    ] {
        let (produced, admission, initial) = produce(machine);
        let artifact = produced.artifact();
        let identifier_path = [field(artifact, "identifier")];
        for loaded in [false, true] {
            let program = if loaded {
                TerminalProgram::load_envelope(&artifact.to_bytes()).expect("external artifact")
            } else {
                TerminalProgram::borrow(artifact.view())
            };
            let entry = program.entry().unwrap();
            let mut argument = TerminalStructuralValue::minted(
                79,
                entry.structural_parameters[0].structural_type,
                Vec::new(),
            );
            // A host may lend a subtree rather than an allocation root. Its
            // original path, like its identity, survives helper frames.
            if loaded {
                argument
                    .path
                    .push(terminal_psi::StructuralPathSegment::FixedIndex(2));
            }
            let arguments = [argument];
            let mut instance = program
                .instantiate(
                    &[],
                    TerminalStructuralInputs {
                        arguments: &arguments,
                        zero_initialized_arguments: &[0],
                        ..Default::default()
                    },
                )
                .expect("host establishes zero-backed borrowed input");
            assert_eq!(
                instance.observe_borrowed_entry(0, &identifier_path),
                Err(TerminalEntryStorageError::NormalReturnRequired)
            );
            let mut meter = TerminalFuelMeter::with_allowance(0);
            assert!(matches!(
                instance.invoke(&mut meter).unwrap(),
                TerminalExecutionStatus::SponsorExhausted(_)
            ));
            assert_eq!(
                instance.observe_borrowed_entry(0, &identifier_path),
                Err(TerminalEntryStorageError::NormalReturnRequired)
            );
            meter.replenish(100_000).unwrap();
            assert_eq!(
                instance.invoke(&mut meter).unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_bytes(
                instance
                    .observe_borrowed_entry(0, &identifier_path)
                    .unwrap(),
                identifier,
            );
            if machine == "configure" {
                let signature_path = [field(artifact, "signature")];
                assert_bytes(
                    instance.observe_borrowed_entry(0, &signature_path).unwrap(),
                    &[0, 9, 0],
                );
                assert!(matches!(
                    instance
                        .observe_borrowed_entry(0, &[signature_path[0], Path::FixedIndex(1)])
                        .unwrap(),
                    TerminalEntryStorageValue::Storage(TerminalStorageContents::Scalar {
                        value: TerminalScalarValue::Integer {
                            value: IntegerValue::Unsigned(9),
                            ..
                        },
                        ..
                    })
                ));
                assert_eq!(
                    instance.observe_borrowed_entry(0, &[signature_path[0], Path::FixedIndex(3)]),
                    Err(TerminalEntryStorageError::UnknownPath)
                );
                assert!(matches!(
                    instance
                        .observe_borrowed_entry(0, &[field(artifact, "revision")])
                        .unwrap(),
                    TerminalEntryStorageValue::Storage(TerminalStorageContents::Scalar {
                        value: TerminalScalarValue::Integer {
                            value: IntegerValue::Unsigned(42),
                            ..
                        },
                        ..
                    })
                ));
            } else {
                let (settings, report) =
                    build_evaluation::terminal_settings::extract(&instance, admission, &initial)
                        .unwrap();
                let expected =
                    package_compilation::build_execution::build_value::EvaluatedBuildSettings {
                        application_identifier: Some(
                            target::ApplicationIdentifier::new(identifier).unwrap(),
                        ),
                        application_intent: Some(target::HostedApplicationIntent::Gui),
                        subsystem: 2,
                        ..Default::default()
                    };
                assert_eq!(settings, expected);
                assert_eq!(
                    report,
                    optimization_core::OptimizationReportRequest::Suppressed
                );
                let TerminalEntryStorageValue::Storage(TerminalStorageContents::Case(selected)) =
                    instance
                        .observe_borrowed_entry(0, &[field(artifact, "subsystem")])
                        .unwrap()
                else {
                    panic!("Build subsystem retains its selected case");
                };
                let module = artifact.semantic_module();
                let selected_case = module
                    .structural_types
                    .iter()
                    .filter_map(|declaration| match &declaration.shape {
                        terminal_psi::StructuralTypeShape::Sum { cases } => Some(cases),
                        _ => None,
                    })
                    .flatten()
                    .find(|case| case.id == selected)
                    .expect("nominal case declaration");
                assert_eq!(module.identities.text(selected_case.identity), Some("Gui"));
            }
            assert_eq!(
                instance.observe_borrowed_entry(1, &identifier_path),
                Err(TerminalEntryStorageError::NotBorrowedEntryArgument { argument_index: 1 })
            );
            assert_eq!(
                instance.observe_borrowed_entry(0, &[]),
                Err(TerminalEntryStorageError::UnknownPath)
            );
            instance.close().unwrap();
            assert_eq!(
                instance.observe_borrowed_entry(0, &identifier_path),
                Err(TerminalEntryStorageError::Closed)
            );
        }
    }
}

fn assert_bytes(observed: TerminalEntryStorageValue<'_>, expected: &[u8]) {
    let TerminalEntryStorageValue::Bytes(bytes) = observed else {
        panic!("expected byte readback");
    };
    assert_eq!(bytes.len(), expected.len());
    let mut output = vec![0; bytes.len()];
    bytes.copy_to(&mut output).unwrap();
    assert_eq!(output, expected);
    assert!(bytes.read(bytes.len()).is_err());
    assert!(bytes.copy_to(&mut []).is_err());
}

#[test]
fn owned_inputs_and_failed_activations_do_not_return_borrowed_storage() {
    for machine in ["consume", "fail_after_write"] {
        let (produced, _, _) = produce(machine);
        let artifact = produced.artifact();
        let identifier_path = [field(artifact, "identifier")];
        let program = TerminalProgram::borrow(artifact.view());
        let arguments = [TerminalStructuralValue::minted(
            79,
            program.entry().unwrap().structural_parameters[0].structural_type,
            Vec::new(),
        )];
        let mut instance = program
            .instantiate(
                &[],
                TerminalStructuralInputs {
                    arguments: &arguments,
                    zero_initialized_arguments: &[0],
                    ..Default::default()
                },
            )
            .unwrap();
        let status = instance
            .invoke(&mut TerminalFuelMeter::unbounded())
            .unwrap();
        if machine == "consume" {
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                instance.observe_borrowed_entry(0, &identifier_path),
                Err(TerminalEntryStorageError::NotBorrowedEntryArgument { argument_index: 0 })
            );
        } else {
            assert!(matches!(status, TerminalExecutionStatus::Crashed(_)));
            assert_eq!(
                instance.observe_borrowed_entry(0, &identifier_path),
                Err(TerminalEntryStorageError::NormalReturnRequired)
            );
        }
    }
}

#[test]
fn build_settings_use_the_existing_validator_after_terminal_execution() {
    for (machine, rejection) in [
        ("build", None),
        ("configure_all_settings", None),
        (
            "configure_invalid_identifier",
            Some("Build.identifier is not a valid application identifier"),
        ),
        (
            "configure_invalid_loader",
            Some("Build.elf_interpreter.linux_arm64 is invalid"),
        ),
        (
            "configure_duplicate_optimization",
            Some("enabled more than once"),
        ),
    ] {
        let (produced, admission, initial) = produce(machine);
        let artifact = produced.artifact();
        for loaded in [false, true] {
            let program = if loaded {
                TerminalProgram::load_envelope(&artifact.to_bytes()).unwrap()
            } else {
                TerminalProgram::borrow(artifact.view())
            };
            let arguments = [TerminalStructuralValue::minted(
                79,
                program.entry().unwrap().structural_parameters[0].structural_type,
                Vec::new(),
            )];
            let host = build_evaluation::terminal_bindings::TerminalBuildBindingHost::new(
                artifact,
                produced.build_bindings(),
                arguments[0].clone(),
            )
            .unwrap();
            let mut instance = program
                .instantiate(
                    &[],
                    TerminalStructuralInputs {
                        arguments: &arguments,
                        zero_initialized_arguments: &[0],
                        ..Default::default()
                    },
                )
                .unwrap();
            for boundary in instance.unbound_imports().collect::<Vec<_>>() {
                let declaration = artifact
                    .semantic_module()
                    .boundary_machines
                    .iter()
                    .find(|declaration| declaration.id == boundary)
                    .unwrap();
                instance
                    .bind_import(boundary, host.adapter(declaration.into()))
                    .unwrap();
            }
            assert!(
                build_evaluation::terminal_settings::extract(&instance, admission, &initial)
                    .unwrap_err()
                    .contains("NormalReturnRequired")
            );
            let mut fuel = TerminalFuelMeter::with_allowance(0);
            for _ in 0..2_000 {
                match instance.invoke(&mut fuel).unwrap() {
                    TerminalExecutionStatus::SponsorExhausted(_) => {
                        assert!(
                            build_evaluation::terminal_settings::extract(
                                &instance, admission, &initial
                            )
                            .unwrap_err()
                            .contains("NormalReturnRequired")
                        );
                        fuel.replenish(1).unwrap();
                    }
                    TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => break,
                    status => panic!("{machine}: {status:?}"),
                }
            }
            assert!(instance.result().is_some(), "{machine} did not complete");
            let result =
                build_evaluation::terminal_settings::extract(&instance, admission, &initial);
            if let Some(rejection) = rejection {
                let error = result.unwrap_err();
                assert!(error.contains(rejection), "{machine}: {error}");
            } else if machine == "build" {
                let (settings, report) = result.unwrap();
                assert_eq!(
                    settings,
                    package_compilation::build_execution::build_value::EvaluatedBuildSettings {
                        application_identifier: Some(
                            target::ApplicationIdentifier::new(b"com.example.omega").unwrap()
                        ),
                        ..Default::default()
                    }
                );
                assert_eq!(
                    report,
                    optimization_core::OptimizationReportRequest::Suppressed
                );
            } else {
                let (settings, report) = result.unwrap();
                assert_eq!(
                    settings.application_identifier,
                    Some(target::ApplicationIdentifier::new(b"com.example.omega").unwrap())
                );
                assert_eq!(settings.application_intent, None);
                assert_eq!(settings.subsystem, 73);
                assert!(settings.freestanding);
                assert!(settings.privileged_services.port_io);
                assert!(!settings.privileged_services.interrupt_table);
                assert_eq!(
                    settings.optimizations,
                    optimization_core::OptimizationSelections::new([
                        optimization_core::Optimization::ControlFlowCleanup,
                    ])
                    .unwrap()
                );
                assert_eq!(
                    report,
                    optimization_core::OptimizationReportRequest::EmitHumanText
                );
                assert!(
                    settings
                        .x86_deployment_claim
                        .admit_for(Some(target::TargetProfile::LinuxX64))
                        .unwrap()
                        .is_some()
                );
                let expected_loader = target::normalize_elf_interpreter_plan(
                    b"/lib/omega-loader.so".to_vec(),
                    target::TargetProfile::LinuxArm64,
                )
                .unwrap();
                assert_eq!(
                    settings
                        .elf_interpreters
                        .explicit(target::TargetProfile::LinuxArm64),
                    Some(&expected_loader)
                );
                assert_eq!(
                    settings
                        .elf_interpreters
                        .explicit(target::TargetProfile::LinuxX64),
                    None
                );
            }
            instance.close().unwrap();
            assert!(
                build_evaluation::terminal_settings::extract(&instance, admission, &initial)
                    .unwrap_err()
                    .contains("Closed")
            );
            drop(instance);
            let requests = host.into_executed().unwrap();
            if machine == "build" {
                assert_eq!(
                    requests
                        .iter()
                        .map(|request| request.slot.as_str())
                        .collect::<Vec<_>>(),
                    [
                        "windows_x86_64::ProgramEntry",
                        "linux_x86_64::ProgramEntry",
                        "linux_arm64::ProgramEntry",
                        "macos_arm64::ProgramEntry"
                    ]
                );
                assert!(requests.iter().all(|request| matches!(
                    &request.implementation,
                    language_semantics::declaration_selection::RootBindingSelection::AuthoredPath(path) if path == "main"
                )));
            } else {
                assert!(requests.is_empty());
            }
        }
    }
}

#[test]
fn full_build_pays_its_shared_sponsor_before_each_semantic_site() {
    use evaluation_accounting::{
        BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
        TerminalStorageScheduleIdentity,
    };
    let (produced, admission, initial) = produce("build");
    let artifact = produced.artifact();
    let program = TerminalProgram::load_envelope(&artifact.to_bytes()).unwrap();
    let execute = |meter: &mut TerminalFuelMeter, completes: bool| {
        let original = TerminalStructuralValue::minted(
            79,
            program.entry().unwrap().structural_parameters[0].structural_type,
            Vec::new(),
        );
        let host = build_evaluation::terminal_bindings::TerminalBuildBindingHost::new(
            artifact,
            produced.build_bindings(),
            original.clone(),
        )
        .unwrap();
        let arguments = [original];
        let mut instance = program
            .instantiate(
                &[],
                TerminalStructuralInputs {
                    arguments: &arguments,
                    zero_initialized_arguments: &[0],
                    ..Default::default()
                },
            )
            .unwrap();
        for boundary in instance.unbound_imports().collect::<Vec<_>>() {
            let declaration = artifact
                .semantic_module()
                .boundary_machines
                .iter()
                .find(|declaration| declaration.id == boundary)
                .unwrap();
            instance
                .bind_import(boundary, host.adapter(declaration.into()))
                .unwrap();
        }
        let result = instance.invoke(meter);
        if completes {
            assert_eq!(
                result.unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            let (settings, _) =
                build_evaluation::terminal_settings::extract(&instance, admission, &initial)
                    .unwrap();
            assert_eq!(
                settings.application_identifier,
                Some(target::ApplicationIdentifier::new(b"com.example.omega").unwrap())
            );
        } else {
            let refusal = format!("{:?}", result.unwrap_err());
            assert!(
                refusal.contains("InvocationCharge")
                    && refusal.contains("AllowanceExhausted(Fuel)"),
                "{refusal}"
            );
            assert!(instance.result().is_none());
            assert!(
                build_evaluation::terminal_settings::extract(&instance, admission, &initial)
                    .unwrap_err()
                    .contains("NormalReturnRequired")
            );
            let usage = meter.usage().clone();
            let effects = instance.effects().len();
            meter.replenish(100_000).unwrap();
            assert!(instance.invoke(meter).is_err());
            assert_eq!(meter.usage(), &usage);
            assert_eq!(
                instance.effects().len(),
                effects,
                "refusal must not replay effects"
            );
        }
        assert_eq!(instance.effects().len(), 8);
        drop(instance);
        assert_eq!(host.into_executed().unwrap().len(), 4);
    };
    let mut baseline = TerminalFuelMeter::unbounded();
    execute(&mut baseline, true);
    let cost = baseline.usage().total_units();
    assert!(cost > 1);
    let measurement = EvaluationMeasurementIdentity::TerminalPsi {
        fuel_schedule: baseline.schedule().identity(),
        storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
    };
    for budget in [2 * cost, 2 * cost - 1] {
        let sponsor = BuildEvaluationSponsor::new_with_measurement(
            BuildEvaluationSponsorLimits::new(budget, 8, 2, 2, 8, 8, 8, 8).unwrap(),
            measurement,
        )
        .unwrap();
        for ordinal in 0..2 {
            let account = sponsor.begin_invocation(measurement, cost).unwrap();
            let mut meter = TerminalFuelMeter::with_invocation(account.clone(), None).unwrap();
            let completes = ordinal == 0 || budget == 2 * cost;
            execute(&mut meter, completes);
            let paid = if completes { cost } else { cost - 1 };
            assert_eq!(meter.usage().total_units(), paid);
            assert_eq!(account.snapshot().unwrap().record().fuel_units, paid);
        }
        assert_eq!(sponsor.consumed_fuel_units(), budget);
    }
}
