//! Every sample app under `samples/` must compile.
//!
//! Samples otherwise have almost no compile coverage — only `cli_mvp` is built
//! by the canary suite and only the dungeon is parse-tested — so they silently
//! bit-rot against language changes. That is exactly what happened when
//! exact-arithmetic (decision 17) became a proof obligation: 22 of the 48
//! samples stopped compiling and nothing noticed. This harness is the guard:
//! one iterating test that compiles every `samples/*/main.omg` for the default
//! target and reports *all* broken samples at once, so a language change that
//! breaks a demo fails the suite the same day.
//!
//! Scope is COMPILE-only on purpose: it catches staleness (the real failure
//! mode) cheaply and broadly, while runtime correctness stays the job of the
//! canary differential suite and the per-target `cli_mvp` execution tests.

use omega_compiler::{CompileOptions, compile};
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under compiler/orchestration/omega-compiler")
        .to_path_buf()
}

#[test]
fn all_samples_compile() {
    let samples_dir = repo_root().join("samples");
    let mut sample_mains: Vec<PathBuf> = fs::read_dir(&samples_dir)
        .expect("samples/ directory should exist")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path().join("main.omg"))
        .filter(|main_path| main_path.is_file())
        .collect();
    sample_mains.sort();

    assert!(
        !sample_mains.is_empty(),
        "expected sample apps under {}",
        samples_dir.display()
    );

    let mut failures: Vec<String> = Vec::new();
    for main_path in &sample_mains {
        let name = main_path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("<unknown>")
            .to_owned();
        let build_dir = std::env::temp_dir().join(format!(
            "omega-sample-compile-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);

        let result = compile(CompileOptions {
            root_path: main_path.clone(),
            build_dir: Some(build_dir.clone()),
            target_name: None,
            write_output: true,
        });
        if let Err(error) = result {
            failures.push(format!("{name}: {error:?}"));
        }
        let _ = fs::remove_dir_all(&build_dir);
    }

    assert!(
        failures.is_empty(),
        "{} of {} samples failed to compile (run the sample's main.omg through \
         omega-cli for the full diagnostic):\n{}",
        failures.len(),
        sample_mains.len(),
        failures.join("\n")
    );
}
