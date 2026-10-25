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

#[test]
fn array_result_storage_outlives_the_completed_instance() {
    use evaluation_accounting::{
        BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
        TerminalStorageScheduleIdentity,
    };
    let (produced, _, _) = produce("array_result");
    let artifact = produced.artifact();
    let execute = |loaded, cell_limit, should_refuse| {
        let program = if loaded {
            TerminalProgram::load_envelope(&artifact.to_bytes()).unwrap()
        } else {
            TerminalProgram::borrow(artifact.view())
        };
        let measurement = EvaluationMeasurementIdentity::TerminalPsi {
            fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
            storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
        };
        let sponsor = BuildEvaluationSponsor::new_with_measurement(
            BuildEvaluationSponsorLimits::new(100_000, 8, 2, 2, cell_limit, 1000, 1000, 1000)
                .unwrap(),
            measurement,
        )
        .unwrap();
        let account = sponsor.begin_invocation(measurement, 100_000).unwrap();
        let mut instance = program
            .instantiate(terminal_interpreter::TerminalInstantiation {
                account: Some(&account),
                ..Default::default()
            })
            .unwrap();
        let mut meter = TerminalFuelMeter::with_invocation(account.clone(), Some(0)).unwrap();
        let result = loop {
            match instance.invoke(&mut meter) {
                Ok(TerminalExecutionStatus::SponsorExhausted(_)) => meter.replenish(1).unwrap(),
                result => break result,
            }
        };
        if should_refuse {
            let refusal = result.unwrap_err();
            assert!(
                format!("{refusal:?}").contains("AllowanceExhausted(LiveStorage)"),
                "{refusal:?}"
            );
            assert!(instance.result().is_none());
            assert_eq!(
                instance.close(),
                Err(terminal_interpreter::TerminalInstanceCloseError::Busy)
            );
            let usage = account.snapshot().unwrap();
            assert_eq!(instance.invoke(&mut meter), Err(refusal));
            assert_eq!(
                account.snapshot().unwrap().record(),
                usage.record(),
                "fault retries cannot repeat charges"
            );
        } else {
            let TerminalExecutionStatus::Complete(TerminalExecutionResult::ScalarArray(returned)) =
                result.unwrap()
            else {
                panic!("array return expected")
            };
            let observed = returned
                .value
                .elements()
                .iter()
                .map(|value| match value {
                    TerminalScalarValue::Integer {
                        value: IntegerValue::Unsigned(value),
                        ..
                    } => *value,
                    _ => panic!("u32 element expected"),
                })
                .collect::<Vec<_>>();
            assert_eq!(
                observed,
                [11, 44, 22],
                "guest copies retain independent values"
            );
            let retained = returned.clone();
            instance.close().unwrap();
            assert_eq!(
                sponsor.live_storage_cells(),
                3,
                "inspection retains the payload after close"
            );
            drop(returned);
            assert_eq!(sponsor.live_storage_cells(), 3);
            drop(retained);
            assert_eq!(sponsor.live_storage_cells(), 0);
        }
        let peak = sponsor.peak_live_cells();
        drop(instance);
        assert_eq!(sponsor.live_storage_cells(), 0);
        peak
    };
    let peak = execute(true, 1000, false);
    assert!(peak > 3);
    assert_eq!(execute(false, peak, false), peak);
    execute(true, peak - 1, true);
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
                .instantiate(terminal_interpreter::TerminalInstantiation {
                    scalar_arguments: &[],
                    structural_inputs: TerminalStructuralInputs {
                        arguments: &arguments,
                        zero_initialized_arguments: &[0],
                        ..Default::default()
                    },
                    ..Default::default()
                })
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
            .instantiate(terminal_interpreter::TerminalInstantiation {
                scalar_arguments: &[],
                structural_inputs: TerminalStructuralInputs {
                    arguments: &arguments,
                    zero_initialized_arguments: &[0],
                    ..Default::default()
                },
                ..Default::default()
            })
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
                .instantiate(terminal_interpreter::TerminalInstantiation {
                    scalar_arguments: &[],
                    structural_inputs: TerminalStructuralInputs {
                        arguments: &arguments,
                        zero_initialized_arguments: &[0],
                        ..Default::default()
                    },
                    ..Default::default()
                })
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
            .instantiate(terminal_interpreter::TerminalInstantiation {
                scalar_arguments: &[],
                structural_inputs: TerminalStructuralInputs {
                    arguments: &arguments,
                    zero_initialized_arguments: &[0],
                    ..Default::default()
                },
                ..Default::default()
            })
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

#[test]
fn full_build_reserves_owned_storage_and_retains_storage_faults() {
    use evaluation_accounting::{
        BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
        TerminalStorageScheduleIdentity,
    };
    let (produced, admission, initial) = produce("build");
    let artifact = produced.artifact();
    let program = TerminalProgram::load_envelope(&artifact.to_bytes()).unwrap();
    let execute = |cell_limit, byte_limit, competing_cells, competing_bytes| {
        let measurement = EvaluationMeasurementIdentity::TerminalPsi {
            fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
            storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
        };
        let sponsor = BuildEvaluationSponsor::new_with_measurement(
            BuildEvaluationSponsorLimits::new(
                100_000, 8, 2, 2, cell_limit, byte_limit, 100_000, 100_000,
            )
            .unwrap(),
            measurement,
        )
        .unwrap();
        let account = sponsor.begin_invocation(measurement, 100_000).unwrap();
        let should_refuse = competing_cells != 0 || competing_bytes != 0;
        let competing = should_refuse.then(|| {
            account
                .reserve_storage(competing_cells, competing_bytes)
                .unwrap()
        });
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
            .instantiate(terminal_interpreter::TerminalInstantiation {
                structural_inputs: TerminalStructuralInputs {
                    arguments: &arguments,
                    zero_initialized_arguments: &[0],
                    ..Default::default()
                },
                account: Some(&account),
                ..Default::default()
            })
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
        let mut meter = TerminalFuelMeter::with_invocation(account.clone(), Some(0)).unwrap();
        let result = loop {
            match instance.invoke(&mut meter) {
                Ok(TerminalExecutionStatus::SponsorExhausted(_)) => {
                    meter.replenish(1).unwrap();
                }
                result => break result,
            }
        };
        if should_refuse {
            let refusal = result.unwrap_err();
            assert!(
                format!("{refusal:?}").contains("AllowanceExhausted(LiveStorage)"),
                "{refusal:?}"
            );
            assert!(instance.result().is_none());
            assert!(
                build_evaluation::terminal_settings::extract(&instance, admission, &initial)
                    .unwrap_err()
                    .contains("NormalReturnRequired")
            );
            assert_eq!(
                instance.close(),
                Err(terminal_interpreter::TerminalInstanceCloseError::Busy)
            );
            let usage = meter.usage().clone();
            let effects = instance.effects().to_vec();
            drop(competing);
            // Capacity is now available, but a fault is not a paused activation.
            assert_eq!(instance.invoke(&mut meter), Err(refusal));
            assert_eq!(meter.usage(), &usage);
            assert_eq!(instance.effects(), effects.as_slice());
        } else {
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
            assert_eq!(instance.effects().len(), 8);
            assert!(
                sponsor.live_payload_bytes() > 0,
                "readback retains the original backing"
            );
            instance.close().unwrap();
            assert_eq!(sponsor.live_payload_bytes(), 0);
            assert_eq!(sponsor.live_storage_cells(), 0);
        }
        drop(instance);
        assert_eq!(
            sponsor.live_payload_bytes(),
            0,
            "all byte owners retire with the instance"
        );
        assert_eq!(
            sponsor.live_storage_cells(),
            0,
            "runtime rows retire with the instance"
        );
        let bindings = host.into_executed();
        if !should_refuse {
            assert_eq!(bindings.unwrap().len(), 4);
        } else if let Err(error) = bindings {
            // Refusal may leave a designated request issued but not committed.
            // That host state must not be published as completed Build output.
            assert!(error.contains("uncommitted request"), "{error}");
        }
        let usage = account.snapshot().unwrap();
        assert!(usage.record().fuel_units > 0);
        *usage.record()
    };
    let baseline = execute(100_000, 100_000, 0, 0);
    let bytes = baseline.peak_live_payload_bytes;
    let cells = baseline.peak_live_storage_cells;
    assert!(bytes > 1);
    assert!(
        cells > 1,
        "source-produced Build must fund its runtime rows"
    );
    assert_eq!(execute(100_000, bytes, 0, 0).peak_live_payload_bytes, bytes);
    assert!(execute(100_000, bytes, 0, 1).peak_live_payload_bytes <= bytes);
    assert_eq!(execute(cells, 100_000, 0, 0).peak_live_storage_cells, cells);
    assert!(execute(cells, 100_000, 1, 0).peak_live_storage_cells <= cells);
}

#[test]
fn empty_frames_are_sponsored_before_entry_and_retire_after_nested_calls() {
    use evaluation_accounting::{
        BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
        TerminalStorageScheduleIdentity,
    };
    let (produced, _, _) = produce("frame_sequence");
    let artifact = produced.artifact();
    let measurement = EvaluationMeasurementIdentity::TerminalPsi {
        fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
        storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
    };
    for (loaded, limit) in [(false, 3), (true, 3), (true, 2)] {
        let program = if loaded {
            TerminalProgram::load_envelope(&artifact.to_bytes()).unwrap()
        } else {
            TerminalProgram::borrow(artifact.view())
        };
        let sponsor = BuildEvaluationSponsor::new_with_measurement(
            BuildEvaluationSponsorLimits::new(100_000, 8, 2, 2, limit, 100, 100, 100).unwrap(),
            measurement,
        )
        .unwrap();
        let account = sponsor.begin_invocation(measurement, 100_000).unwrap();
        let mut instance = program
            .instantiate(terminal_interpreter::TerminalInstantiation {
                account: Some(&account),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            sponsor.live_storage_cells(),
            1,
            "the empty root frame owns one cell"
        );
        let mut meter = TerminalFuelMeter::with_invocation(account.clone(), Some(0)).unwrap();
        let result = loop {
            match instance.invoke(&mut meter) {
                Ok(TerminalExecutionStatus::SponsorExhausted(_)) => {
                    assert!((1..=limit).contains(&sponsor.live_storage_cells()));
                    meter.replenish(1).unwrap();
                }
                result => break result,
            }
        };
        if limit == 3 {
            assert_eq!(
                result.unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                account.snapshot().unwrap().record().peak_live_storage_cells,
                3
            );
            assert_eq!(
                sponsor.live_storage_cells(),
                1,
                "only the returned root activation remains"
            );
            instance.close().unwrap();
        } else {
            let refusal = result.unwrap_err();
            assert!(
                format!("{refusal:?}").contains("AllowanceExhausted(LiveStorage)"),
                "{refusal:?}"
            );
            assert_eq!(sponsor.live_storage_cells(), 2);
            let usage = *account.snapshot().unwrap().record();
            assert_eq!(instance.invoke(&mut meter), Err(refusal));
            assert_eq!(*account.snapshot().unwrap().record(), usage);
            assert!(instance.result().is_none());
            assert_eq!(
                instance.close(),
                Err(terminal_interpreter::TerminalInstanceCloseError::Busy)
            );
        }
        drop(instance);
        assert_eq!(sponsor.live_storage_cells(), 0);

        let competing = account.reserve_storage(limit, 0).unwrap();
        let refusal = program
            .instantiate(terminal_interpreter::TerminalInstantiation {
                account: Some(&account),
                ..Default::default()
            })
            .err()
            .expect("entry reservation must refuse before creating the root frame");
        assert!(
            format!("{refusal:?}").contains("AllowanceExhausted(LiveStorage)"),
            "{refusal:?}"
        );
        assert_eq!(sponsor.live_storage_cells(), limit);
        drop(competing);
        assert_eq!(sponsor.live_storage_cells(), 0);
    }
}

#[test]
fn a_fault_after_a_host_effect_cannot_reenter_the_same_call() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct RejectAfterEffect<'host> {
        signature: terminal_interpreter::TerminalImportSignature,
        calls: &'host AtomicUsize,
    }
    impl terminal_interpreter::TerminalImportAdapter for RejectAfterEffect<'_> {
        fn implemented_signature(&self) -> terminal_interpreter::TerminalImportSignature {
            self.signature.clone()
        }
    }
    impl terminal_interpreter::TerminalEffectHandler for RejectAfterEffect<'_> {
        fn handle_effect(
            &mut self,
            _: &terminal_interpreter::TerminalEffect,
        ) -> Result<(), terminal_interpreter::TerminalEffectRejection> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Err(terminal_interpreter::TerminalEffectRejection::new(
                "host failed after its effect",
            ))
        }
    }
    let (produced, _, _) = produce("configure_build_boundary");
    let artifact = produced.artifact();
    let program = TerminalProgram::borrow(artifact.view());
    let arguments = [TerminalStructuralValue::minted(
        79,
        program.entry().unwrap().structural_parameters[0].structural_type,
        Vec::new(),
    )];
    let calls = AtomicUsize::new(0);
    let mut instance = program
        .instantiate(terminal_interpreter::TerminalInstantiation {
            structural_inputs: TerminalStructuralInputs {
                arguments: &arguments,
                zero_initialized_arguments: &[0],
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
    let imports = instance.unbound_imports().collect::<Vec<_>>();
    assert_eq!(imports.len(), 1);
    let boundary = artifact
        .semantic_module()
        .boundary_machines
        .iter()
        .find(|declaration| declaration.id == imports[0])
        .unwrap();
    instance
        .bind_import(
            imports[0],
            RejectAfterEffect {
                signature: boundary.into(),
                calls: &calls,
            },
        )
        .unwrap();
    let mut meter = TerminalFuelMeter::unbounded();
    let failure = instance.invoke(&mut meter).unwrap_err();
    assert!(format!("{failure:?}").contains("host failed after its effect"));
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    let usage = meter.usage().clone();
    let effects = instance.effects().to_vec();
    assert_eq!(instance.invoke(&mut meter), Err(failure));
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(meter.usage(), &usage);
    assert_eq!(instance.effects(), effects.as_slice());
    assert!(instance.result().is_none());
    assert_eq!(
        instance.close(),
        Err(terminal_interpreter::TerminalInstanceCloseError::Busy)
    );
}
