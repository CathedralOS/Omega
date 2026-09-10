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
fn wrapping_array_receiver_storage_preserves_scalar_calls_at_every_fuel_pause() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "omega-policy-array-storage-{}-{stamp}",
        std::process::id(),
    )));
    fs::create_dir(&fixture.0).unwrap();
    let main = fixture.0.join("main.omg");
    // The array is part of the exact receiver storage shape. Indexed array
    // update production is a separate current boundary; scalar field operations
    // exercise this receiver and its ordinary helper without claiming array access.
    let source = r#"
use omega::language::core::external_binding;

boundary trait Trace { machine record(value: u64); }
windows_x86_64 machine trace_binding() -> Binding<12, 11, 0> {
    Binding::DllImport {
        import: DllImport::PeByName { library: "kernel32.dll", export: "ExitProcess" },
    }
}
machine trace_leaf(value: u64) satisfies Trace::record via trace_binding();

data Main {
    values: [u64 in Wrapping; 16];
    observed: u64 in Wrapping;
}
machine Main::main(&mut self) {
    self.observed = 18446744073709551615;
    Trace::record(self.observed as u64);
    self.advance();
    Trace::record(self.observed as u64);
    self.observed = self.observed + 1;
    Trace::record(self.observed as u64);
}
machine Main::advance(&mut self) {
    self.observed = self.observed + 1;
}

"#;
    fs::write(&main, source).unwrap();
    fs::write(
        fixture.0.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.application("policy-array-storage");
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
    let report = compile(request).unwrap_or_else(|diagnostics| {
        panic!(
            "arithmetic-policy array publication failed; artifacts at {}:\n{}",
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
    let receiver_type = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == receiver.structural_type)
        .unwrap();
    let terminal_psi::StructuralTypeShape::Record { fields } = &receiver_type.shape else {
        panic!("receiver retains its authored record shape");
    };
    assert_eq!(
        fields.len(),
        2,
        "unused array storage must remain in the receiver"
    );
    assert!(
        fields.iter().any(|field| {
            let terminal_psi::StructuralFieldType::Structural(array) = field.field_type else {
                return false;
            };
            module.structural_types.iter().any(|declaration| {
                declaration.id == array
                    && matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::FixedArray { length: 16, .. }
                    )
            })
        }),
        "receiver retains the complete sixteen-element array"
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
    assert_eq!(handler.0, [u64::MAX, 0, 1]);
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
            [u64::MAX, 0, 1],
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

#[test]
fn wrapping_array_reads_cannot_implicitly_erase_arithmetic_policy() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "omega-policy-array-erasure-{}-{stamp}",
        std::process::id()
    )));
    fs::create_dir(&fixture.0).unwrap();
    let main = fixture.0.join("main.omg");
    fs::write(
        &main,
        r#"
data Store { values: [u64 in Wrapping; 16]; }
machine Store::read(&self) -> u64 {
    let erased: u64 = self.values[0];
    erased
}
"#,
    )
    .unwrap();
    let result = compile(
        CompileRequest::new(CompileOptions {
            root_path: main,
            build_dir: Some(fixture.0.join("build")),
            target_name: None,
        })
        .with_requested_product(RequestedCompileProduct::Check),
    );
    let Err(diagnostics) = result else {
        panic!("an implicit bare binding must not erase stored Wrapping meaning");
    };
    let text = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        text.contains("implicit domain weakening") && text.contains("Wrapping"),
        "{text}"
    );
}
