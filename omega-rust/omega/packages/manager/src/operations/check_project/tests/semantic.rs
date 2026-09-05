use super::*;
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
machine Main::main(&mut self) { self.console.exit_process(70); }
"#,
    );
    let (entry, closure) = project.prepare("application/entry.omg").into_review_parts();
    let checked = compile_resolved_package_candidate_for_check(
        &closure.for_exact_target(TARGET),
        &project.0.join("checked"),
        &entry,
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
    assert_empty_directory(&project.0.join("checked"));
}
