use omega_compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileRequest, RequestedCompileProduct, compile,
    compile_to_checked,
};
use std::fs;
use std::path::{Path, PathBuf};

const CALLBACK_USE: &str = r#"
data CallbackProvider { }

machine CallbackProvider::call(message: u64)
satisfies WindowProcedure::call
{
}

data Main {
    registrar: WindowRegistrar;
    specification: Spread<ForeignRecord>;
}

machine Main::main(&mut self) {
    WindowRegistrar::register<CallbackProvider::call, CallbackProvider::call>(&self.specification);
}
"#;

struct Fixture {
    root: PathBuf,
    main: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(5)
            .expect("Omega repository root");
        let root = repository.join(format!(
            "source/library/std/tests/.callback-terminal-custody-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create callback-custody fixture");

        let source = fs::read_to_string(
            repository.join("source/library/std/tests/callback_materialization_closure.omg"),
        )
        .expect("read callback materialization fixture");
        let source = source
            .replacen(
                "boundary trait WindowProcedure {",
                "boundary trait WindowProcedure: Calling<RegistrarPolicy> {",
                1,
            )
            .replacen(
                "machine call(message: u64) -> u64;",
                "machine call(message: u64);",
                1,
            )
            .replace(
                "data Main { }\nmachine Main::main(&mut self) { }\n",
                CALLBACK_USE.trim_start_matches('\n'),
            );
        assert!(source.contains("CallbackProvider::call"));

        let main = root.join("main.omg");
        fs::write(&main, source).expect("write callback-custody source");
        fs::write(
            root.join("build.omg"),
            r#"machine build(builder: &mut Build) {
    builder.application("callback-terminal-custody");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
}
"#,
        )
        .expect("write callback-custody build policy");

        Self { root, main }
    }

    fn request(&self, product: RequestedCompileProduct, tag: &str) -> CompileRequest {
        CompileRequest::new(CompileOptions {
            root_path: self.main.clone(),
            build_dir: Some(self.root.join(format!("build-{tag}"))),
            target_name: Some("windows_x64".to_owned()),
            write_output: false,
        })
        .with_requested_product(product)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn assert_custody_diagnostic(diagnostics: &[psi_diagnostics::Diagnostic], product: &str) {
    assert_eq!(
        diagnostics.len(),
        1,
        "unexpected diagnostics: {diagnostics:#?}"
    );
    let message = diagnostics[0].message.as_str();
    assert!(
        message.contains(product),
        "unexpected diagnostic: {message}"
    );
    assert!(
        message.contains("2 validated callback placement(s)"),
        "unexpected diagnostic: {message}"
    );
    assert_eq!(
        message.matches("WindowProcedure::call").count(),
        2,
        "the diagnostic must name every retained callback row: {message}"
    );
    assert!(
        message.contains("canonical Terminal callback-use custody is not implemented"),
        "unexpected diagnostic: {message}"
    );
}

#[test]
fn canonical_terminal_handoff_rejects_unconsumed_callback_placements() {
    let fixture = Fixture::new();
    let checked = compile_to_checked(&fixture.main, Some("windows_x64"))
        .expect("callback program should reach checked compilation");
    assert_eq!(checked.callback_placements().len(), 2);

    compile(fixture.request(RequestedCompileProduct::Check, "check"))
        .expect("check-only compilation retains callback placements without executing them");

    let terminal = compile(fixture.request(RequestedCompileProduct::TerminalArtifact, "terminal"))
        .expect_err("Terminal production cannot silently discard callback placements");
    assert_custody_diagnostic(&terminal, "terminal-artifact");

    let native = compile(fixture.request(RequestedCompileProduct::NativeArtifact, "native"))
        .expect_err("native production cannot silently discard callback placements");
    assert_custody_diagnostic(&native, "native-artifact");
}
