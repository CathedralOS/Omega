// Differential throwaway test for game_of_life sample.
// Runs interpreter and native, asserts they agree on exit code 70.
// DELETE this file when the sample is complete.

use omega_compiler::{CompileOptions, compile, compile_to_checked};
use omega_interpreter::interpret;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join(".git").exists())
        .expect("repo root not found")
        .to_path_buf()
}

#[cfg(windows)]
fn executable_name() -> &'static str {
    "omega-program.exe"
}

#[cfg(not(windows))]
fn executable_name() -> &'static str {
    "omega-program"
}

#[cfg(windows)]
#[test]
fn game_of_life_interpreter_matches_native() {
    let sample = repo_root().join("samples").join("game_of_life");
    let main_path = sample.join("main.omg");

    // --- Interpreter path ---
    let checked = compile_to_checked(&main_path, None)
        .expect("game_of_life should compile to checked trees for interpreter");

    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "interpreter should support game_of_life, got error: {:?}",
        outcome.error
    );
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter should exit 70 for game_of_life, got {}",
        outcome.exit_code
    );

    // --- Native path ---
    let build_dir = std::env::temp_dir()
        .join(format!("omega-gol-diff-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x64".to_owned()),
        write_output: true,
    })
    .expect("game_of_life should compile to native");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("game_of_life native exe should run");

    let native_code = output.status.code().expect("native should have an exit code");
    assert_eq!(
        native_code, 70,
        "native should exit 70 for game_of_life, got {}",
        native_code
    );

    // Differential assertion: interpreter and native must agree.
    assert_eq!(
        outcome.exit_code, native_code,
        "differential mismatch: interpreter exited {} but native exited {}",
        outcome.exit_code, native_code
    );

    let _ = fs::remove_dir_all(&build_dir);
}
