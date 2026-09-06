use compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileRequest, RequestedCompileProduct, compile,
};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn composed_unit_arguments_reach_published_terminal_and_native_provider_custody() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "omega-composed-unit-arguments-{}-{stamp}",
        std::process::id(),
    )));
    fs::create_dir(&fixture.0).unwrap();
    let main = fixture.0.join("main.omg");
    fs::write(
        &main,
        r#"
use omega::language::core::external_binding;

boundary trait Console { machine write(value: u8); }
windows_x86_64 machine write_binding() -> Binding<12, 11, 0> {
    Binding::DllImport {
        import: DllImport::PeByName { library: "kernel32.dll", export: "ExitProcess" },
    }
}
machine write_leaf(value: u8) satisfies Console::write via write_binding();

machine identity(value: u8) -> u8 { value }
machine forward(value: u8) { Console::write(value); }
machine relay(value: u8) { forward(identity(value)); }
data Main {}
machine Main::main(&mut self) {
    let selected: u64 = 1u64;
    transition selected { 1u64 -> yes() _ -> no() }
    state yes() { relay(identity(7u8)); }
    state no() { relay(identity(9u8)); }
}
"#,
    )
    .unwrap();
    fs::write(
        fixture.0.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.application("composed-unit-arguments");
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
    .with_requested_product(RequestedCompileProduct::TerminalArtifact)
    .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly);
    let report = compile(request).unwrap_or_else(|diagnostics| {
        panic!(
            "composed calls must survive complete compiler publication:\n{}",
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
    let module = terminal_codec::decode_module(retained.artifact().semantic_bytes()).unwrap();
    assert!(
        module
            .machines
            .iter()
            .filter(|machine| machine.attachment.is_none())
            .count()
            >= 3
    );
    assert!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind, terminal_psi::OperationKind::CallUnit { ref arguments, .. }
                    if arguments.len() == 1
            ))
    );
    retained
        .native_realization_proposal()
        .expect("native provider custody")
        .validate_for_artifact(retained.artifact())
        .expect("exact provider proposal");
    // This is target-independent artifact/custody validation, not execution of
    // the Windows image on the current development host.
}
