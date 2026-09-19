use super::super::compile_resolved_package_candidate_for_check;
use super::{Project, TARGET, assert_empty_directory};
use package_compilation::AcceptedSemanticBindingRole;

#[test]
fn retained_check_root_uses_final_consumer_bindings_and_requested_entry() {
    let project = Project::new();
    project.write(
        "console/build.omg",
        "machine build(builder: &mut Build) { builder.package(\"ordinary-console\"); }\n",
    );
    project.write(
        "console/main.omg",
        r#"
pub boundary trait Console {
    machine exit_process(return_code: i32)
    reaches Console;
}
pub data ConsoleNativeProvider {}
windows_x86_64 machine ConsoleNativeProvider::exit_process(return_code: i32)
    satisfies Console::exit_process
    via Binding::CompilerIntrinsic;
"#,
    );
    project.write(
        "application/build.omg",
        r#"
machine build(builder: &mut Build) {
    builder.application("console-consumer");
    builder.depend_as("ordinary_console", Source::Path { location: "../console" });
    builder.select_provider<Console, ConsoleNativeProvider>();
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
}
"#,
    );
    project.write(
        "application/main.omg",
        "the unselected source must not enter either pass\n",
    );
    project.write(
        "application/entry.omg",
        r#"
use ordinary_console::main;
use omega::language::core::service;
data Main { console: Service<Console> in Bound; }
machine Main::main(&mut self) reaches Console { self.console.exit_process(70); }
"#,
    );
    let expected_file_bytes = ["application/build.omg", "application/entry.omg"]
        .map(|path| {
            std::fs::read(project.0.join(path))
                .expect("read authored source fixture")
                .len() as u64
        })
        .into_iter()
        .sum::<u64>();
    let (entry, closure, _) = project.prepare("application/entry.omg").into_review_parts();
    let snapshot = build_evaluation::BuildSnapshotRequest::scoped(
        std::iter::empty(),
        package_compilation::BuildSourceCaptureRequest::new(["build.omg", "entry.omg"].map(
            |path| {
                (
                    path.as_bytes().to_vec(),
                    package_compilation::BuildSourceCaptureObligation::Required,
                )
            },
        ))
        .expect("declare only the selected root sources"),
    );
    let checked = compile_resolved_package_candidate_for_check(
        &closure.for_exact_target(TARGET),
        &project.0.join("checked"),
        &entry,
        Some(&snapshot),
    )
    .expect("retain the final binding pass at the requested entry");
    let bindings = checked.resolved_semantic_bindings().collect::<Vec<_>>();
    assert_eq!(
        bindings.len(),
        1,
        "the preliminary pass has no consumed bindings"
    );
    assert_eq!(
        bindings[0].role(),
        AcceptedSemanticBindingRole::ConsoleExitProcessI32
    );
    assert_eq!(checked.selected_target_profile(), Some(TARGET));
    let inventory = checked
        .build_observation_summary()
        .unwrap()
        .captured_source_inventory()
        .unwrap();
    assert_eq!(
        inventory.entry_count(),
        3,
        "the final binding pass retains the narrowed root inventory; dependency builds have no entry.omg and must keep their own inventories",
    );
    assert_eq!(
        inventory.file_bytes(),
        expected_file_bytes,
        "retained input bytes contain exactly authored build.omg and entry.omg"
    );
    assert_empty_directory(&project.0.join("checked"));
}
