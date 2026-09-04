use omega_compiler::compile_to_checked;
use omega_effects::provider_plan::ProviderBinding;
use omega_target::{ForeignLocatorCandidate, TargetProfile};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct TemporaryProgram(PathBuf);

impl TemporaryProgram {
    fn new(source: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "omega-evaluated-via-binding-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("create evaluated-via fixture");
        fs::write(directory.join("main.omg"), source).expect("write evaluated-via fixture");
        Self(directory)
    }

    fn main(&self) -> PathBuf {
        self.0.join("main.omg")
    }
}

impl Drop for TemporaryProgram {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn evaluates_exact_via_binding_into_provider_identity() {
    let fixture = TemporaryProgram::new(
        r#"
use omega::language::core::external_binding;


boundary trait Console {
    machine write(value: u8);
}

windows_x86_64 machine write_binding() -> Binding<12, 11, 0> {
    Binding::DllImport {
        import: DllImport::PeByName {
            library: "kernel32.dll",
            export: "ExitProcess",
        },
    }
}

machine write_leaf(value: u8)
    satisfies Console::write
    via write_binding();

data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let checked =
        compile_to_checked(&fixture.main(), Some("windows_x86_64")).unwrap_or_else(|diagnostics| {
            panic!(
                "ordinary via binding should compile:\n{}",
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });

    assert_eq!(checked.evaluated_via_bindings().rows().len(), 1);
    let imports = checked
        .provider_plans()
        .iter()
        .flat_map(|plan| plan.rows.iter())
        .filter_map(|row| match &row.binding {
            ProviderBinding::Import { evaluated } => Some(evaluated),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [evaluated] = imports.as_slice() else {
        panic!("one evaluated import provider row expected");
    };
    assert_eq!(evaluated.locator().target(), TargetProfile::WindowsX64);
    assert_eq!(
        evaluated.locator().locator(),
        &ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"ExitProcess".to_vec(),
        }
    );
    assert_eq!(
        evaluated.receipt().locator_identity_digest(),
        evaluated.locator().identity_digest()
    );
    assert_ne!(evaluated.receipt().identity_digest(), [0; 32]);
}

#[test]
fn evaluates_exact_syscall_binding_into_existing_provider_identity() {
    let fixture = TemporaryProgram::new(
        r#"
use omega::language::core::external_binding;


boundary trait Process {
    machine exit(code: i32);
}

linux_x86_64 machine exit_binding() -> Binding<0, 0, 0> {
    Binding::Syscall { number: 60 }
}

machine exit_leaf(code: i32)
    satisfies Process::exit
    via exit_binding();

data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let checked =
        compile_to_checked(&fixture.main(), Some("linux_x86_64")).unwrap_or_else(|diagnostics| {
            panic!(
                "ordinary syscall via binding should compile:\n{}",
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });

    let [evaluated_row] = checked.evaluated_via_bindings().rows() else {
        panic!("one evaluated syscall row expected");
    };
    let Some(evaluated) = evaluated_row.evaluated().as_syscall() else {
        panic!("ordinary syscall must retain its evaluated receipt distinctly");
    };
    assert_eq!(evaluated.target(), TargetProfile::LinuxX64);
    assert_eq!(evaluated.number(), 60);
    assert_eq!(
        evaluated.receipt().locator_identity_digest(),
        evaluated.identity_digest()
    );
    assert_ne!(evaluated.receipt().identity_digest(), [0; 32]);

    let syscalls = checked
        .provider_plans()
        .iter()
        .flat_map(|plan| &plan.rows)
        .filter_map(|row| match row.binding {
            ProviderBinding::Syscall { number } => Some(number),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(syscalls, [60]);
}

#[test]
fn ordinary_syscall_binding_rejects_wrong_target_and_used_widths() {
    for (target, widths, expected) in [
        (
            "windows_x86_64",
            "0, 0, 0",
            "not applicable to selected target",
        ),
        (
            "linux_x86_64",
            "1, 0, 0",
            "Syscall ObjectLength must be zero",
        ),
    ] {
        let source = format!(
            r#"
use omega::language::core::external_binding;

boundary trait Process {{
    machine exit(code: i32);
}}

{target} machine exit_binding() -> Binding<{widths}> {{
    Binding::Syscall {{ number: 60 }}
}}

machine exit_leaf(code: i32)
    satisfies Process::exit
    via exit_binding();

data Main {{}}
machine Main::main(&mut self) {{}}
"#,
        );
        let fixture = TemporaryProgram::new(&source);
        let diagnostics = compile_to_checked(&fixture.main(), Some(target))
            .expect_err("invalid ordinary syscall binding must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.to_string().contains(expected)),
            "expected `{expected}` in {diagnostics:#?}",
        );
    }
}

#[test]
fn local_binding_lookalike_does_not_enter_the_evaluated_vocabulary() {
    let fixture = TemporaryProgram::new(
        r#"
data Binding<const A: u64, const B: u64, const C: u64> {
    case Syscall(number: u64);
}


boundary trait Process {
    machine exit(code: i32);
}

linux_x86_64 machine exit_binding() -> Binding<0, 0, 0> {
    Binding::Syscall { number: 60 }
}

machine exit_leaf(code: i32)
    satisfies Process::exit
    via exit_binding();

data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let diagnostics = compile_to_checked(&fixture.main(), Some("linux_x86_64"))
        .expect_err("local Binding lookalike must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .to_string()
                .contains("unique compiler-owned Binding")
        }),
        "unexpected diagnostics: {diagnostics:#?}",
    );
}
