//! `ProcessExit::exit_process(code)` ends the process with the operand
//! written, natively and in Psi, and nothing after it runs. Only the
//! canonical requirement Core declares does this: a package's own boundary
//! trait spelled `ProcessExit` is an ordinary boundary requirement, and with
//! no provider the program is refused.
//!
//! The spec rules: `wiki/spec/language/process_exit.md` (the canonical
//! requirement ends the process with its code), and
//! `wiki/spec/language/effects.md`, "Process-exit reach". Each program stages
//! a package that depends on the bundled `system`, which provides the exit, and
//! runs its entry through the native and Psi engines.

#[path = "support/project.rs"]
mod project;

/// Stage `main` with a Build that depends on the bundled `system` and binds
/// `main` on every hosted target, run it both ways, and return the exit
/// status and everything printed.
fn run(name: &str, main: &str) -> [(Option<i32>, String); 2] {
    let application = name.replace('-', "_");
    let build = format!(
        "use main.omg as app;
use {{omega}}/build/dependency.omg;
use {{omega}}/language/core/process_exit.omg;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application {{ name: \"{application}\" }};
    let adapter: DependencyDescription = |dependency|::from_source(Source::Bundled {{ name: \"system\" }});
    let allowed: [BoundaryServiceReference; 1] = [Build::service::<|process_exit|::ProcessExit>(builder)];
    _ = |dependency|::add(builder, adapter, @allowed[..]);
    Build::bind(builder, windows_x86_64::ProgramEntry, |app|::main);
    Build::bind(builder, windows_arm64::ProgramEntry, |app|::main);
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    Build::bind(builder, linux_arm64::ProgramEntry, |app|::main);
    Build::bind(builder, macos_arm64::ProgramEntry, |app|::main);
}}
"
    );
    run_build(name, &build, main)
}

/// Stage `build` and `main` as one package, run it both ways, and return the
/// exit status and everything printed.
fn run_build(name: &str, build: &str, main: &str) -> [(Option<i32>, String); 2] {
    ["native", "psi"].map(|engine| {
        let scratch = project::stage(name, &[("build.omg", build), ("main.omg", main)]);
        let output = project::omega(scratch.path(), &["run", "--engine", engine, "build.omg"]);
        let report = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        eprintln!("{report}");
        (output.status.code(), report)
    })
}

#[track_caller]
fn exits_with(code: i32, runs: [(Option<i32>, String); 2]) {
    for ((status, report), engine) in runs.into_iter().zip(["native", "Psi"]) {
        assert_eq!(status, Some(code), "{engine}: {report}");
        let expected = format!("{engine} exit {code}");
        assert!(report.lines().any(|line| line == expected), "{report}");
    }
}

const QUIT: &str = "use {omega}/language/core/process_exit.omg;
machine quit(code: i32, other: i32) reaches |process_exit|::ProcessExit { |process_exit|::ProcessExit::exit_process(code); }
public machine main() reaches |process_exit|::ProcessExit { quit(3, 5); }
";

/// `quit(3, 5)` exits with its first formal, 3; the twin that passes
/// `other` to the exit exits with 5.
#[test]
fn the_exit_ends_the_process_with_the_operand_written() {
    exits_with(3, run("exit-operand", QUIT));

    let other = QUIT.replace("exit_process(code)", "exit_process(other)");
    exits_with(5, run("exit-other-operand", &other));
}

/// Nothing after a grouped exit statement runs: `quit` would return 7, and
/// the process instead ends with the 4 passed to the exit.
#[test]
fn nothing_after_an_exit_runs() {
    exits_with(
        4,
        run(
            "exit-grouped",
            "use {omega}/language/core/process_exit.omg;
machine quit(code: i32) -> i32 reaches |process_exit|::ProcessExit { (|process_exit|::ProcessExit::exit_process(code)); 7 }
public machine main() reaches |process_exit|::ProcessExit { let ignored: i32 = quit(4); }
",
        ),
    );
}

/// A boundary trait of the package's own, spelled like Core's, is an
/// ordinary requirement. Nothing provides it, so the program is refused
/// instead of exiting.
#[test]
fn a_same_spelled_local_requirement_is_not_the_exit() {
    for (status, report) in run(
        "exit-local-requirement",
        "boundary trait ProcessExit { machine exit_process(return_code: i32) -> i32 reaches ProcessExit crashes none terminates; }
machine stop(code: i32) -> i32 reaches ProcessExit { ProcessExit::exit_process(code) }
public machine main() reaches ProcessExit { let ignored: i32 = stop(6); }
",
    ) {
        assert_ne!(status, Some(0), "{report}");
        assert!(
            report.contains("boundary requirement `ProcessExit::exit_process` has no provider on target"),
            "{report}"
        );
        assert!(!report.contains("native exit 6"), "{report}");
    }
}

/// A package may provide the exit itself. Its provider stands for the
/// requirement, so it must end the process with the status its caller
/// passed: "the exact exit argument survive[s]" (process_exit.md). The twin
/// differs only in the argument the provider hands to the core primitive.
#[test]
fn a_provider_of_the_exit_passes_on_the_status_it_received() {
    let build = "use main.omg as app;
use {omega}/language/core/process_exit.omg;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: \"exit_provider\" };
    Build::select_provider::<|process_exit|::ProcessExit, |app|::LocalExit>(builder, windows_x86_64);
    Build::select_provider::<|process_exit|::ProcessExit, |app|::LocalExit>(builder, windows_arm64);
    Build::select_provider::<|process_exit|::ProcessExit, |app|::LocalExit>(builder, linux_x86_64);
    Build::select_provider::<|process_exit|::ProcessExit, |app|::LocalExit>(builder, linux_arm64);
    Build::select_provider::<|process_exit|::ProcessExit, |app|::LocalExit>(builder, macos_arm64);
    Build::bind(builder, windows_x86_64::ProgramEntry, |app|::main);
    Build::bind(builder, windows_arm64::ProgramEntry, |app|::main);
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    Build::bind(builder, linux_arm64::ProgramEntry, |app|::main);
    Build::bind(builder, macos_arm64::ProgramEntry, |app|::main);
}
";
    let honest = "use {omega}/language/core/never.omg;
use {omega}/language/core/process_exit.omg;
use {omega}/language/core/process_exit_native.omg;
public LocalExit satisfies |process_exit|::ProcessExit {
    machine exit_process(return_code: i32) -> |never|::Never
    reaches |process_exit|::ProcessExit
    crashes none
    terminates {
        |process_exit_native|::ProcessExitPrimitive(return_code);
    }
}
public machine main() reaches |process_exit|::ProcessExit { |process_exit|::ProcessExit::exit_process(7); }
";
    exits_with(7, run_build("exit-provider", build, honest));

    let changed = honest.replace(
        "ProcessExitPrimitive(return_code)",
        "ProcessExitPrimitive(0)",
    );
    for (status, report) in run_build("exit-provider-changed", build, &changed) {
        assert!(
            report.contains("does not pass on the arguments the machine received"),
            "{report}"
        );
        assert_ne!(status, Some(0), "{report}");
        assert!(!report.contains("native exit 0"), "{report}");
        assert!(!report.contains("Psi exit 0"), "{report}");
    }
}
