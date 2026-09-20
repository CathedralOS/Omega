//! A source-produced foreign call carrying mixed scalar and record arguments
//! reaches target-lowering translation validation on the linux_x86_64 host: a
//! record actual established by an ordinary call result lowers into a
//! structural-home boundary argument, then the normalized-foreign signature
//! rebuild — which still realizes only borrowed pointer transport — rejects it
//! closed instead of silently transporting ownership.

#[test]
fn mixed_scalar_and_record_arguments_stop_at_call_argument_validation() {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    run_linux_record_counterparty();
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    eprintln!("skip: record foreign arguments require Linux x86-64");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn run_linux_record_counterparty() {
    use super::{
        Fixture, admit_imports, terminal_authority_permission_policy, terminal_authority_policy,
    };
    use compiler::{RetainedNativeRealizationRequest, SourceEvaluatedImportSettlement};
    use native_realization as native;

    let fixture = Fixture::with_source(
        "record-foreign-arguments",
        "linux_x86_64",
        r#"use omega::language::core::service;
use omega::language::core::external_binding;

pub data Pair {
    first: u64;
    second: u64;
}

pub boundary trait Aggregate {
    machine combine(tag: u64, pair: Pair) -> u64;
    machine check(answer: u64);
}

linux_x86_64 machine combine_binding() -> Binding<15, 11, 7> {
    Binding::DllImport {
        import: DllImport::ElfVersioned {
            object: "libagg-probe.so",
            symbol: "agg_combine",
            version: "OMEGA_1",
        },
    }
}

linux_x86_64 machine check_binding() -> Binding<15, 9, 7> {
    Binding::DllImport {
        import: DllImport::ElfVersioned {
            object: "libagg-probe.so",
            symbol: "agg_check",
            version: "OMEGA_1",
        },
    }
}

machine make_pair() -> Pair {
    Pair { first: 2u64, second: 22u64 }
}

machine combine_leaf(tag: u64, pair: Pair) -> u64
    satisfies Aggregate::combine via combine_binding();
machine check_leaf(answer: u64) satisfies Aggregate::check via check_binding();

data Main { boundary: Service<Aggregate>; }
machine Main::main(&mut self) reaches Aggregate {
    let pair: Pair = make_pair();
    let answer: u64 = self.boundary.combine(20u64, pair);
    self.boundary.check(answer);
}
"#,
        r#"machine build(builder: &mut Build) {
    builder.application("record-foreign-arguments");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}
"#,
    );

    let retained = fixture.compile_terminal();
    let admissions = admit_imports(&retained, 0x4147_4752_0001);
    assert_eq!(admissions.len(), 2);
    let imports = admissions
        .iter()
        .map(|admission| {
            SourceEvaluatedImportSettlement::new(&admission.execution, &admission.same_stack)
        })
        .collect::<Vec<_>>();
    let policy = terminal_authority_policy(&retained);
    let permission_policy = terminal_authority_permission_policy(&retained);
    let interpreter = target::normalize_elf_interpreter_plan(
        b"/lib64/ld-linux-x86-64.so.2".to_vec(),
        target::TargetProfile::LinuxX64,
    )
    .expect("canonical Linux x86-64 interpreter");
    let (_, diagnostics) = compiler::realize_retained_native_artifact(
        retained,
        RetainedNativeRealizationRequest {
            profile: &proof_admission::AdmissionProfile::default(),
            optimization_selections:
                &optimization_core::PostTerminalOptimizationSelections::default(),
            terminal_authority_policy: policy,
            accepted_package_terminal_authority_permission_policy:
                native::current_terminal_authority_permission_policy(),
            terminal_authority_permission_policy: Some(permission_policy),
            image_request: native::ExecutableImageEmissionRequest::dynamic_elf(interpreter),
            imports: &imports,
        },
    )
    .unwrap_err();
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .to_string()
                .contains("StructuralCallArgumentMismatch")
        }),
        "an owned record actual must refuse inside normalized-foreign signature rebuild, not earlier in lowering or later in emission; diagnostics: {diagnostics:?}",
    );
}
