use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct, compile};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue, TerminalStructuralValue,
};

struct Fixture(PathBuf);

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
        TerminalExecution::start_artifact_with_structural_arguments(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
            &[TerminalStructuralValue {
                opaque_identity: 1,
                structural_type: receiver.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
        )
        .expect("published artifact independently verifies and reloads")
    };
    let complete = TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit);
    let mut execution = execute();
    let mut meter = TerminalFuelMeter::with_allowance(1000);
    let mut handler = Trace::default();
    assert_eq!(
        execution
            .resume_with_effect_handler(&mut meter, &mut handler)
            .unwrap(),
        complete
    );
    assert_eq!(handler.0, [10, 20, expected]);
    let total = meter.usage().total_units();
    for allowance in 0..total {
        let mut execution = execute();
        let mut meter = TerminalFuelMeter::with_allowance(allowance);
        let mut handler = Trace::default();
        assert!(matches!(
            execution
                .resume_with_effect_handler(&mut meter, &mut handler)
                .unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        meter.replenish(total - allowance).unwrap();
        assert_eq!(
            execution
                .resume_with_effect_handler(&mut meter, &mut handler)
                .unwrap(),
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
