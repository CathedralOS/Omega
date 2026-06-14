// Throwaway test: verify the game_of_life sample compiles and exits 70.
// DELETE this file when the sample is complete.

use omega_compiler::{CompileOptions, compile};
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
fn game_of_life_sample_exits_70() {
    let sample = repo_root().join("samples").join("game_of_life");
    let main_path = sample.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-game-of-life-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let result = compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x64".to_owned()),
        write_output: true,
    });

    match result {
        Err(diagnostics) => {
            panic!(
                "game_of_life should compile, but got diagnostics:\n{}",
                diagnostics
                    .iter()
                    .map(|d| d.to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
        Ok(_) => {}
    }

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("game_of_life exe should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "game_of_life should exit 70 (correct final board checksum), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}
