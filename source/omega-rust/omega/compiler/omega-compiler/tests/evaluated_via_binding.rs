use omega_compiler::compile_to_checked;
use omega_effects::provider_plan::ProviderBinding;
use omega_target::{ForeignLocatorCandidate, TargetProfile};
use std::fs;
use std::path::PathBuf;

struct TemporaryProgram(PathBuf);

impl TemporaryProgram {
    fn new(source: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "omega-evaluated-via-binding-{}",
            std::process::id()
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

target windows_x86_64 {
}

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
