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
//! runs it with `omega run --both build.omg`.

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Stdio};

/// Stage `main` with a Build that depends on the bundled `system` and binds
/// `main` on every hosted target, run it both ways, and return the exit
/// status and everything printed.
fn run(name: &str, main: &str) -> (Option<i32>, String) {
    let library = support::repository_root().join("source/library/system");
    let library = library.to_string_lossy().replace('\\', "/");
    let application = name.replace('-', "_");
    let build = format!(
        "machine build(builder: @mutable Build) {{
    Build::application(builder, \"{application}\");
    let registration: DependencyRegistration = Build::dependency(builder, dependency\\from_source(Source::Path {{ location: \"{library}\" }}));
    Build::bind(builder, windows_x86_64\\ProgramEntry, main);
    Build::bind(builder, linux_x86_64\\ProgramEntry, main);
    Build::bind(builder, linux_arm64\\ProgramEntry, main);
    Build::bind(builder, macos_arm64\\ProgramEntry, main);
}}
"
    );
    run_build(name, &build, main)
}

/// Stage `build` and `main` as one package, run it both ways, and return the
/// exit status and everything printed.
fn run_build(name: &str, build: &str, main: &str) -> (Option<i32>, String) {
    let scratch = TemporaryDirectory::create(name, false).unwrap();
    std::fs::write(scratch.path().join("build.omg"), build).unwrap();
    std::fs::write(scratch.path().join("main.omg"), main).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run", "--both", "build.omg"])
        .current_dir(scratch.path())
        .stdin(Stdio::null())
        .output()
        .expect("omega runs");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("{report}");
    (output.status.code(), report)
}

#[track_caller]
fn exits_with(code: i32, report: &str) {
    for receipt in [
        format!("native exit {code}"),
        format!("interp exit: {code}"),
    ] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

const QUIT: &str = "use omega\\language\\core\\ProcessExit;
machine quit(code: i32, other: i32) reaches ProcessExit { ProcessExit::exit_process(code); }
machine main() reaches ProcessExit { quit(3, 5); }
";

/// `quit(3, 5)` exits with its first formal, 3; the twin that passes
/// `other` to the exit exits with 5.
#[test]
fn the_exit_ends_the_process_with_the_operand_written() {
    let (status, report) = run("exit-operand", QUIT);
    assert_eq!(status, Some(3), "{report}");
    exits_with(3, &report);

    let other = QUIT.replace("exit_process(code)", "exit_process(other)");
    let (status, report) = run("exit-other-operand", &other);
    assert_eq!(status, Some(5), "{report}");
    exits_with(5, &report);
}

/// Nothing after a grouped exit statement runs: `quit` would return 7, and
/// the process instead ends with the 4 passed to the exit.
#[test]
fn nothing_after_an_exit_runs() {
    let (status, report) = run(
        "exit-grouped",
        "use omega\\language\\core\\ProcessExit;
machine quit(code: i32) -> i32 reaches ProcessExit { (ProcessExit::exit_process(code)); 7 }
machine main() reaches ProcessExit { let ignored: i32 = quit(4); }
",
    );
    assert_eq!(status, Some(4), "{report}");
    exits_with(4, &report);
}

/// A boundary trait of the package's own, spelled like Core's, is an
/// ordinary requirement. Nothing provides it, so the program is refused
/// instead of exiting.
#[test]
fn a_same_spelled_local_requirement_is_not_the_exit() {
    let (status, report) = run(
        "exit-local-requirement",
        "boundary trait ProcessExit { machine exit_process(return_code: i32) -> i32 reaches ProcessExit crashes none terminates; }
machine stop(code: i32) -> i32 reaches ProcessExit { ProcessExit::exit_process(code) }
machine main() reaches ProcessExit { let ignored: i32 = stop(6); }
",
    );
    assert_ne!(status, Some(0), "{report}");
    assert!(
        report
            .contains("boundary requirement `ProcessExit::exit_process` has no provider on target"),
        "{report}"
    );
    assert!(!report.contains("native exit 6"), "{report}");
}

/// A package may provide the exit itself. Its provider stands for the
/// requirement, so it must end the process with the status its caller
/// passed: "the exact exit argument survive[s]" (process_exit.md). The twin
/// differs only in the argument the provider hands to the core primitive.
#[test]
fn a_provider_of_the_exit_passes_on_the_status_it_received() {
    let build = "machine build(builder: @mutable Build) {
    Build::application(builder, \"exit_provider\");
    Build::select_provider::<ProcessExit, LocalExit>(builder, windows_x86_64);
    Build::select_provider::<ProcessExit, LocalExit>(builder, linux_x86_64);
    Build::select_provider::<ProcessExit, LocalExit>(builder, linux_arm64);
    Build::select_provider::<ProcessExit, LocalExit>(builder, macos_arm64);
    Build::bind(builder, windows_x86_64\\ProgramEntry, main);
    Build::bind(builder, linux_x86_64\\ProgramEntry, main);
    Build::bind(builder, linux_arm64\\ProgramEntry, main);
    Build::bind(builder, macos_arm64\\ProgramEntry, main);
}
";
    let honest = "use omega\\language\\core\\Never;
use omega\\language\\core\\ProcessExit;
use omega\\language\\core\\ProcessExitPrimitive;
LocalExit satisfies ProcessExit {
    machine exit_process(return_code: i32) -> Never
    reaches ProcessExit
    crashes none
    terminates {
        ProcessExitPrimitive(return_code);
    }
}
machine main() reaches ProcessExit { ProcessExit::exit_process(7); }
";
    let (status, report) = run_build("exit-provider", build, honest);
    assert_eq!(status, Some(7), "{report}");
    exits_with(7, &report);

    let changed = honest.replace("ProcessExitPrimitive(return_code)", "ProcessExitPrimitive(0)");
    let (status, report) = run_build("exit-provider-changed", build, &changed);
    assert!(
        report.contains("does not pass on the arguments the machine received"),
        "{report}"
    );
    assert_ne!(status, Some(0), "{report}");
    assert!(!report.contains("native exit 0"), "{report}");
    assert!(!report.contains("interp exit: 0"), "{report}");
}
