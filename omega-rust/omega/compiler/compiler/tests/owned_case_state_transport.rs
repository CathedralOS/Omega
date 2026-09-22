use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct, compile};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue, TerminalStructuralValue,
};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

struct Fixture(PathBuf);

#[test]
fn scalar_callees_establish_their_own_cases_without_structural_inputs() {
    // Copy cases are observed by the consumer; affine cases exercise transfer
    // and implicit disposal without relying on the separate owned-match gap.
    for (properties, inspection) in [
        (
            "[copy]",
            "transition value { Choice::First -> (7) Choice::Second -> (9) }",
        ),
        ("", "result"),
    ] {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "omega-scalar-local-case-{}-{stamp}",
            std::process::id()
        )));
        fs::create_dir(&fixture.0).unwrap();
        let main = fixture.0.join("main.omg");
        fs::write(
            &main,
            format!(
                r#"
data Choice {properties} {{ case First; case Second; }}
machine inspect(value: Choice, result: u64) -> u64 {{
    {inspection}
}}
machine helper(selected: bool) -> u64 {{
    transition selected {{ true -> first() false -> second() }}
    state first() -> u64 {{ inspect(Choice::First, 7) }}
    state second() -> u64 {{ inspect(Choice::Second, 9) }}
}}
machine evaluate(selected: bool) -> u64 {{ helper(selected) }}
"#
            ),
        )
        .unwrap();
        let checked = compiler::compile_to_checked(compiler::CheckedCompileRequest::new(
            &main,
            Some("linux_x86_64"),
        ))
        .expect("scalar caller and locally established cases check");
        let artifact = terminal_production::TerminalProductionRequest::new(
            &checked,
            TerminalMachineSelection::Name("evaluate"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("callee-local storage is not a caller input")
        .into_artifact();
        let input = terminal_psi_to_abstract_operations::lower_artifact(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: artifact.semantic_bytes(),
                proof_bytes: artifact.proof_bytes(),
                obligation_ledger_bytes: None,
            },
            &proof_admission::AdmissionProfile::default(),
        )
        .map(|admitted| {
            admitted
                .into_optimization_artifact()
                .into_optimization_input()
        })
        .expect("local case calls lower through the ordinary abstract-operation path");
        let verified = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
            input,
            terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
        )
        .unwrap();
        optimization_unit_semantics::validate_psi_optimization_unit(verified.unit())
            .expect("abstract replay also distinguishes callee locals from boundary inputs");
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let callee = entry
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation.kind {
                terminal_psi::OperationKind::Call { callee, .. } => Some(callee),
                _ => None,
            })
            .expect("evaluate calls helper through the scalar-only boundary");
        let helper = module
            .machines
            .iter()
            .find(|machine| machine.id == callee)
            .unwrap();
        assert!(helper.structural_parameters.is_empty());
        assert!(helper.entry_claims.is_empty());
        assert!(
            helper
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(
                    operation.kind,
                    terminal_psi::OperationKind::EstablishScalarCase { .. }
                )),
            "the scalar callee owns its local case producers"
        );
        for (selected, expected) in [(true, 7), (false, 9)] {
            let mut execution = TerminalExecution::start_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &proof_admission::AdmissionProfile::default(),
                &[TerminalScalarValue::Boolean(selected)],
                TerminalStructuralInputs::default(),
            )
            .expect("independent verification requires no invented structural input");
            assert_eq!(
                execution
                    .resume(
                        &mut TerminalFuelMeter::with_allowance(1000),
                        &mut terminal_interpreter::AcceptTerminalEffects,
                    )
                    .unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                    TerminalScalarValue::Integer {
                        scalar_type: semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            64
                        )
                        .unwrap(),
                        value: semantic_vocabulary::IntegerValue::Unsigned(expected),
                    }
                ))
            );
        }

        // A local's declaration is not its establishment. Removing the actual
        // producer must still fail the callee's independent machine checks.
        let mut missing_establishment = module.clone();
        let block = missing_establishment
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .find(|block| {
                block.operations.iter().any(|operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::EstablishScalarCase { .. }
                    )
                })
            })
            .expect("one local case producer");
        let producer = block
            .operations
            .iter()
            .position(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::EstablishScalarCase { .. }
                )
            })
            .unwrap();
        block.operations.remove(producer);
        let error = terminal_verifier::validate_module(&missing_establishment).unwrap_err();
        assert!(
            matches!(
                error,
                terminal_verifier::ModuleError::StructuralCallResultPlaceMismatch(_)
            ),
            "{error:?}"
        );

        // The other side of the boundary: inspect really does need its owned
        // case argument, so disguising that call as scalar-only must reject.
        let mut missing_argument = module.clone();
        let call = missing_argument
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .find(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::CallStructuralScalar { .. }
                )
            })
            .expect("owned case argument call");
        let terminal_psi::OperationKind::CallStructuralScalar {
            callee,
            arguments,
            erased_arguments,
            requirement_obligations,
            crash_continuations,
            ..
        } = call.kind.clone()
        else {
            unreachable!()
        };
        call.kind = terminal_psi::OperationKind::Call {
            callee,
            arguments,
            erased_arguments,
            erased_proof_arguments: Vec::new(),
            requirement_obligations,
            crash_continuations,
        };
        assert!(matches!(
            terminal_verifier::validate_module(&missing_argument),
            Err(terminal_verifier::ModuleError::CallTargetHasStructuralContract { .. })
        ));
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

#[test]
fn owned_error_kind_state_transport_preserves_calls_and_every_fuel_pause() {
    for (selected, expected) in [(true, 70), (false, 71)] {
        assert_owned_state_transport(selected, expected);
    }
}

fn assert_owned_state_transport(selected: bool, expected: u64) {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "omega-owned-case-state-{}-{stamp}",
        std::process::id(),
    )));
    fs::create_dir(&fixture.0).unwrap();
    let main = fixture.0.join("main.omg");
    let source = r#"
use omega::language::core::external_binding;
use omega::language::std::filesystem;

boundary trait Trace { machine record(value: u64); }
windows_x86_64 machine trace_binding() -> Binding<12, 11, 0> {
    Binding::DllImport {
        import: DllImport::PeByName { library: "kernel32.dll", export: "ExitProcess" },
    }
}
machine trace_leaf(value: u64) satisfies Trace::record via trace_binding();

machine make_error_kind(selected: bool) -> ErrorKind {
    transition selected { true -> missing() false -> other() }
    state missing() -> ErrorKind { ErrorKind::NotFound }
    state other() -> ErrorKind { ErrorKind::Other }
}

data Main {}
machine Main::main(&mut self) {
    Trace::record(10);
    let kind: ErrorKind = make_error_kind(SELECTED);
    Trace::record(20);
    transition { _ -> inspect(kind) }
    state inspect(&mut self, kind: ErrorKind) {
        transition kind {
            ErrorKind::NotFound -> missing()
            _ -> other()
        }
    }
    state missing(&mut self) { Trace::record(70); }
    state other(&mut self) { Trace::record(71); }
}

"#;
    fs::write(
        &main,
        source.replace("SELECTED", if selected { "true" } else { "false" }),
    )
    .unwrap();
    fs::write(
        fixture.0.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.application("owned-case-state");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
}
"#,
    )
    .unwrap();
    let request = CompileRequest::new(CompileOptions {
        root_path: main,
        build_dir: Some(fixture.0.join("build")),
        target_name: Some("windows_x86_64".to_owned()),
    })
    .with_requested_product(RequestedCompileProduct::TerminalArtifact);
    let report = compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| {
            panic!(
                "owned case state publication failed; artifacts at {}:\n{}",
                fixture.0.display(),
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        });
    let retained = report
        .into_retained_terminal_artifact()
        .expect("Terminal product");
    let artifact = retained.artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert!(
        entry.ranked_scc.is_none(),
        "no fabricated termination witness"
    );
    let [receiver] = entry.structural_parameters.as_slice() else {
        panic!("entry retains its original mutable receiver")
    };
    assert_eq!(
        receiver.access,
        terminal_psi::StructuralAccess::MutableBorrow
    );
    let kind = module
        .structural_types
        .iter()
        .find(|declaration| declaration.identity == "named(name(ErrorKind))")
        .expect("actual filesystem ErrorKind type retained");
    let terminal_psi::StructuralTypeShape::Sum { cases } = &kind.shape else {
        panic!("ErrorKind remains a nominal sum, not a scalar tag");
    };
    assert!(cases.iter().any(|case| case.identity == "NotFound"));
    assert!(
        entry
            .blocks
            .iter()
            .flat_map(|block| &block.structural_parameters)
            .any(|parameter| {
                parameter.structural_type == kind.id
                    && parameter.access == terminal_psi::StructuralAccess::Owned
                    && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Affine
            }),
        "inspect retains the owned affine ErrorKind state parameter"
    );
    let execute = || {
        TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs {
                arguments: &[TerminalStructuralValue {
                    opaque_identity: 1,
                    structural_type: receiver.structural_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                }],
                ..Default::default()
            },
        )
        .expect("published artifact independently verifies and reloads")
    };
    let complete = TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit);
    let mut execution = execute();
    let mut meter = TerminalFuelMeter::with_allowance(1000);
    let mut handler = Trace::default();
    assert_eq!(
        execution.resume(&mut meter, &mut handler).unwrap(),
        complete
    );
    assert_eq!(handler.0, [10, 20, expected]);
    let total = meter.usage().total_units();
    for allowance in 0..total {
        let mut execution = execute();
        let mut meter = TerminalFuelMeter::with_allowance(allowance);
        let mut handler = Trace::default();
        assert!(matches!(
            execution.resume(&mut meter, &mut handler).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        meter.replenish(total - allowance).unwrap();
        assert_eq!(
            execution.resume(&mut meter, &mut handler).unwrap(),
            complete
        );
        assert_eq!(
            handler.0,
            [10, 20, expected],
            "effects at fuel split {allowance}"
        );
        assert_eq!(meter.usage().total_units(), total);
    }
}

#[derive(Default)]
struct Trace(Vec<u64>);

impl TerminalEffectHandler for Trace {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            panic!("only the authored trace boundary is observable")
        };
        let [
            TerminalScalarValue::Integer {
                value: semantic_vocabulary::IntegerValue::Unsigned(value),
                ..
            },
        ] = arguments.as_slice()
        else {
            panic!("trace retains its exact unsigned argument")
        };
        self.0.push(u64::try_from(*value).unwrap());
        Ok(())
    }
}
