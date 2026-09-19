use super::{CheckedChildExecution, PreparedCheckedSource};
use checked_interpreter::InterpretOptions;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PREPARED_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct PreparedFixture {
    root: std::path::PathBuf,
    main: std::path::PathBuf,
}

impl PreparedFixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-prepared-checked-source-{}-{}",
            std::process::id(),
            NEXT_PREPARED_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).expect("create prepared checked-source fixture");
        let main = root.join("main.omg");
        fs::write(&main, "const ANSWER: u32 = 42;\n").expect("write prepared checked-source main");
        fs::write(
            root.join("build.omg"),
            r#"machine build(builder: &mut Build) {
builder.application("prepared-checked-source");
transition builder.target {
    TargetProfile::WindowsX86_64 -> windows(builder)
    _ -> other(builder)
}
state windows(builder: &mut Build) {
    builder.subsystem = Subsystem::Gui;
}
state other(builder: &mut Build) {
    builder.subsystem = Subsystem::Console;
}
}
"#,
        )
        .expect("write prepared checked-source build");
        Self { root, main }
    }
}

impl Drop for PreparedFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn checked_request_preserves_targetless_and_exact_target_selection() {
    let fixture = PreparedFixture::new();
    fs::remove_file(fixture.root.join("build.omg")).expect("remove target-dependent build");
    let request = super::CheckedCompileRequest::new(&fixture.main, None);
    assert!(request.package_inputs.is_none());
    assert!(request.build_dir.is_none());
    assert!(request.filesystem_sponsor.is_none());
    assert!(request.evaluation_sponsor.is_none());
    let targetless = super::compile_to_checked(request).expect("targetless request checks");
    assert_eq!(targetless.selected_target_profile(), None);
    assert_eq!(targetless.selected_native_target(), None);
    assert!(targetless.selected_program_entry().is_none());
    let exact = super::compile_to_checked(super::CheckedCompileRequest::new(
        &fixture.main,
        Some("windows_x86_64"),
    ))
    .expect("exact target request checks");
    assert_eq!(
        exact.selected_target_profile(),
        Some(target::TargetProfile::WindowsX64)
    );
    assert_eq!(
        exact.selected_native_target(),
        Some(target::TargetProfile::WindowsX64.native_target())
    );
    assert_eq!(targetless.source_file_count(), 1);
    assert!(
        exact.source_file_count() > targetless.source_file_count(),
        "exact target selection seeds the hosted entry contract"
    );
    assert!(
        !fixture.root.join("build").exists(),
        "preparing a filesystem scope must not create staging without build execution"
    );
}

#[test]
fn checked_request_rejects_unknown_target_before_source_preparation() {
    let fixture = PreparedFixture::new();
    let diagnostics = super::compile_to_checked(super::CheckedCompileRequest::new(
        &fixture.root.join("absent.omg"),
        Some("not-an-omega-target"),
    ))
    .expect_err("unknown target rejects before missing source is read");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("not-an-omega-target"));
}

#[test]
fn terminal_and_interpreter_share_canonical_boundary_calls() {
    use typed_trees::statement::StatementNode;
    for forwarding in [false, true] {
        let fixture = PreparedFixture::new();
        fs::write(
            &fixture.main,
            r#"
boundary trait Sink { machine emit(value: i32); machine echo(value: i32) -> i32; }
data SinkProvider {}
machine SinkProvider::emit(RECEIVERvalue: i32) satisfies Sink::emit {}
machine SinkProvider::echo(RECEIVERvalue: i32) -> i32 satisfies Sink::echo { value }
data Main { sink: Sink; }
machine Main::main(&mut self) reaches Sink { self.sink.emit(7); }
machine Main::query(&mut self) -> i32 reaches Sink { self.sink.echo(35) }
"#
            .replace("RECEIVER", if forwarding { "service: Sink, " } else { "" }),
        )
        .unwrap();
        let checked = super::compile_to_checked(super::CheckedCompileRequest::new(
            &fixture.main,
            Some("macos_arm64"),
        ))
        .expect("selected source checks");
        let main = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .unwrap();
        let statements = checked.machine_states(main)[0].statement_nodes;
        let StatementNode::Call(call) = &checked.statement_table.statements(statements)[0] else {
            panic!("canonical boundary call");
        };
        assert_eq!(call.target.as_str(), "emit");
        assert!(!call.receiver.is_empty());
        assert!(!checked.facts.boundary_adapter_dispatch.is_empty());
        assert!(std::ptr::eq(checked.terminal_production_trees(), &*checked));
        let source = checked
            .pre_selected_dispatch_source_trees(&checked.typed)
            .unwrap();
        assert!(matches!(source, std::borrow::Cow::Borrowed(_)));
        let outcome = checked_interpreter::interpret_entry(
            &checked,
            "Main::query",
            &[],
            InterpretOptions::default(),
        );
        assert_eq!(outcome.error, None);
        assert_eq!(outcome.exit_code, 35);
        let statement_outcome = checked_interpreter::interpret_entry(
            &checked,
            "Main::main",
            &[],
            InterpretOptions::default(),
        );
        assert_eq!(statement_outcome.error, None);
        let produced = terminal_production::TerminalProductionRequest::new(
            checked.terminal_production_trees(),
            "Main::main",
        )
        .produce_artifact();
        if forwarding {
            // Terminal still has no layout/Unit plan for an ordinary trait-valued
            // provider parameter. Interpreter forwarding does not grant one.
            assert!(matches!(produced,
                Err(terminal_production::TerminalArtifactProductionError::Lowering(
                    checked_trees_to_lowered_psi::LoweringError::InvalidUnitMachinePlan { ref machine, .. }
                )) if machine == "SinkProvider::emit"));
        } else {
            produced.unwrap().validate().unwrap();
        }
    }
}

#[test]
fn selected_boundary_adapter_identity_precedes_builtin_spelling() {
    let fixture = PreparedFixture::new();
    fs::write(
        &fixture.main,
        r#"
boundary trait Arithmetic { machine max(left: i32, right: i32) -> i32; }
data Provider {}
machine Provider::first(left: i32, right: i32) -> i32
satisfies Arithmetic::max { left }
data Main { arithmetic: Arithmetic; }
machine Main::main(&mut self) -> i32 reaches Arithmetic {
self.arithmetic.max(7, 35)
}
"#,
    )
    .unwrap();
    let checked = super::compile_to_checked(super::CheckedCompileRequest::new(
        &fixture.main,
        Some("macos_arm64"),
    ))
    .expect("named boundary source checks");
    let outcome = checked_interpreter::interpret_entry(
        &checked,
        "Main::main",
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}

#[test]
fn selected_boundary_adapter_guard_subject_runs_once() {
    let fixture = PreparedFixture::new();
    fs::write(
        &fixture.main,
        r#"
boundary trait Switch { machine flip(value: &mut bool) -> bool; }
data Provider {}
machine Provider::flip(value: &mut bool) -> bool satisfies Switch::flip {
value = !value;
value
}
data Main { switch: Switch; flag: bool; }
machine Main::main(&mut self) -> i32 reaches Switch {
transition self.switch.flip(&mut self.flag) {
    false -> (1)
    true -> (35)
}
}
"#,
    )
    .unwrap();
    let checked = super::compile_to_checked(super::CheckedCompileRequest::new(
        &fixture.main,
        Some("macos_arm64"),
    ))
    .expect("guard adapter source checks");
    let outcome = checked_interpreter::interpret_entry(
        &checked,
        "Main::main",
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 35);
}

#[test]
fn prepared_source_checkpoint_preserves_standalone_child_identity_and_siblings() {
    let fixture = PreparedFixture::new();
    let standalone = super::compile_to_checked(super::CheckedCompileRequest::new(
        &fixture.main,
        Some("windows_x86_64"),
    ))
    .expect("standalone Windows child should compile");
    let main = fixture.main.clone();
    let (windows, linux, windows_again) =
        crate::checking::compile_thread::run_on_compile_thread(move || {
            let prepared = PreparedCheckedSource::prepare(&main, None)
                .expect("prepare checked source checkpoint");
            let windows = prepared
                .clone()
                .compile_child(CheckedChildExecution::exact_target(
                    target::TargetProfile::WindowsX64,
                ))
                .expect("compile Windows child from prepared source");
            let linux = prepared
                .clone()
                .compile_child(CheckedChildExecution::exact_target(
                    target::TargetProfile::LinuxX64,
                ))
                .expect("compile Linux child from prepared source");
            let windows_again = prepared
                .compile_child(CheckedChildExecution::exact_target(
                    target::TargetProfile::WindowsX64,
                ))
                .expect("recompile Windows child after sibling");
            Ok((windows, linux, windows_again))
        })
        .expect("spawn prepared source compiler thread");

    assert_eq!(standalone, windows);
    assert_eq!(windows, windows_again);
    assert_eq!(windows.subsystem(), 2);
    assert_eq!(
        linux.selected_target_profile(),
        Some(target::TargetProfile::LinuxX64),
    );
    assert_ne!(linux.subsystem(), windows.subsystem());
    assert_eq!(
        windows.application_intent(),
        Some(build_evaluation::HostedApplicationIntent::Gui)
    );
    assert_eq!(
        linux.application_intent(),
        Some(build_evaluation::HostedApplicationIntent::Console)
    );
}

#[test]
fn retained_checked_request_preserves_identity_and_rejects_foreign_inputs() {
    let fixture = PreparedFixture::new();
    let request = || super::CheckedCompileRequest::new(&fixture.main, Some("windows_x86_64"));
    let standalone = super::compile_to_checked(request()).expect("independent checked child");
    let mut retained = None;
    let discovery = super::compile_to_checked(super::CheckedCompileRequest {
        prepared_source_output: Some(&mut retained),
        ..request()
    })
    .expect("retain discovery source preparation");
    let prepared = retained.expect("successful checked request publishes preparation");
    assert_eq!(discovery, standalone);

    let mut foreign_root = request();
    foreign_root.root_path = fixture.root.join("different.omg");
    let diagnostics = prepared
        .clone()
        .compile_to_checked(foreign_root)
        .expect_err("a retained frontier belongs to its original entry");
    assert!(diagnostics[0].message.contains("root does not match"));

    let mut failed_output = Some(prepared.clone());
    let invalid_target = super::compile_to_checked(super::CheckedCompileRequest {
        target_name: Some("not-an-omega-target".to_owned()),
        prepared_source_output: Some(&mut failed_output),
        ..request()
    });
    assert!(invalid_target.is_err());
    assert!(
        failed_output.is_none(),
        "failed validation clears old output"
    );

    failed_output = Some(prepared.clone());
    let missing_source = super::compile_to_checked(super::CheckedCompileRequest {
        root_path: fixture.root.join("missing.omg"),
        prepared_source_output: Some(&mut failed_output),
        ..request()
    });
    assert!(missing_source.is_err());
    assert!(
        failed_output.is_none(),
        "failed compilation clears old output"
    );

    let identity = semantic_vocabulary::PackageKeyIdentity::from_digest([9; 32]).unwrap();
    let mut foreign_inputs = request();
    foreign_inputs.package_inputs = Some(
        package_compilation::PackageCompilationInputs::new_package(
            identity,
            vec![package_compilation::PackageSourceBinding::new(
                identity,
                "different-source-inputs",
                fixture.root.clone(),
            )],
            vec![],
        )
        .unwrap(),
    );
    let diagnostics = prepared
        .clone()
        .compile_to_checked(foreign_inputs)
        .expect_err("a valid but foreign source projection cannot replace retained inputs");
    assert!(
        diagnostics[0]
            .message
            .contains("source inputs do not match")
    );

    let checked = prepared
        .compile_to_checked(request())
        .expect("consume retained frontier");
    assert_eq!(checked, standalone);
    checked
        .verify_current_source_consumption()
        .expect("fresh checked source custody");
}
