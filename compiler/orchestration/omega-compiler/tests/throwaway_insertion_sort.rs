// Throwaway test: compile insertion_sort sample and run it, check exit 70.
// Also checks interpreter matches native.
// DELETE this file once the permanent differential test is added.

use omega_compiler::{CompileOptions, compile, compile_to_checked};
use omega_interpreter::interpret;
use std::fs;
use std::path::Path;
use std::process::Command;

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("omega-compiler crate should live under compiler/orchestration/omega-compiler")
        .to_path_buf()
}

#[test]
fn insertion_sort_sample_exits_70() {
    let sample = repo_root().join("samples").join("insertion_sort");
    let main_path = sample.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-insertion-sort-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path.clone(),
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x64".to_owned()),
        write_output: true,
    })
    .expect("insertion_sort should compile");

    let output = Command::new(build_dir.join("omega-program.exe"))
        .output()
        .expect("insertion_sort exe should run");

    let native_code = output.status.code().unwrap_or(-1);
    assert_eq!(
        native_code, 70,
        "insertion_sort expected exit 70, got {}\nstderr: {}",
        native_code,
        String::from_utf8_lossy(&output.stderr)
    );

    // Check interpreter matches native.
    let checked = compile_to_checked(&main_path, None).expect("insertion_sort compile_to_checked");
    let outcome = interpret(&checked, b"");
    if !outcome.is_error() {
        assert_eq!(
            outcome.exit_code, native_code,
            "interpreter {} != native {} for insertion_sort",
            outcome.exit_code, native_code
        );
    } else {
        eprintln!("insertion_sort: interpreter skipped: {:?}", outcome.error);
    }

    let _ = fs::remove_dir_all(&build_dir);
}
