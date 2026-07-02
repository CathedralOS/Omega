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
//! Two guards:
//!  * `all_samples_compile` — every `samples/*/main.omg` must compile (catches
//!    staleness like the decision-17 break).
//!  * `samples_with_documented_exit_run_correctly` — every sample whose comment
//!    states `Expected exit: N` is compiled, RUN, and its exit asserted. This
//!    catches runtime miscompiles that compile cleanly: stack_vm exited 71 vs 70
//!    for weeks because nothing ran the samples. Now a native miscompile in a
//!    demo fails the suite the same day. A sample may ALSO state
//!    `Expected output contains: <text>`, and its stdout is asserted to contain
//!    that text — exit code alone passes even when a RENDERER silently draws
//!    nothing (a broken carrier render), so the renderers assert a glyph they draw.

use omega_compiler::{CompileOptions, compile};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[cfg(windows)]
fn executable_name() -> &'static str {
    "omega-program.exe"
}

#[cfg(not(windows))]
fn executable_name() -> &'static str {
    "omega-program"
}

/// Parse a `// Expected exit: N` annotation (any casing) from a sample's source.
/// The COLON is required: a comment merely MENTIONING the phrase ("this sample
/// has no Expected exit annotation") must not opt a forever-running app into
/// the auto-run set on a garbage code scraped from later prose (bitten by
/// samples/window_app, which waits for a human to close its window).
fn documented_expected_exit(source: &str) -> Option<i32> {
    let lower = source.to_ascii_lowercase();
    let after = &lower[lower.find("expected exit:")? + "expected exit:".len()..];
    let digits: String = after
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// Parse a `// Expected output contains: <text>` annotation from a sample's source.
/// Returns the (original-case) substring that the sample's stdout must contain --
/// the smoke test that a RENDERER actually drew something, since exit code alone
/// passes even when a renderer silently emits nothing.
fn documented_expected_output(source: &str) -> Option<String> {
    const MARKER: &str = "expected output contains:";
    // Lower-cased copy is ASCII-length-preserving, so the byte index is valid in the
    // original source -- extract the substring from the original to keep its case.
    let index = source.to_ascii_lowercase().find(MARKER)? + MARKER.len();
    let line = source[index..].lines().next()?.trim();
    (!line.is_empty()).then(|| line.to_owned())
}

fn sample_mains() -> Vec<PathBuf> {
    let samples_dir = repo_root().join("samples");
    let mut mains: Vec<PathBuf> = fs::read_dir(&samples_dir)
        .expect("samples/ directory should exist")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path().join("main.omg"))
        .filter(|main_path| main_path.is_file())
        .collect();
    mains.sort();
    assert!(!mains.is_empty(), "expected sample apps under {}", samples_dir.display());
    mains
}

fn sample_name(main_path: &Path) -> String {
    main_path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("<unknown>")
        .to_owned()
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under compiler/orchestration/omega-compiler")
        .to_path_buf()
}

#[test]
fn all_samples_compile() {
    let sample_mains = sample_mains();
    let mut failures: Vec<String> = Vec::new();
    for main_path in &sample_mains {
        let name = sample_name(main_path);
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

#[test]
fn samples_with_documented_exit_run_correctly() {
    let sample_mains = sample_mains();
    let mut failures: Vec<String> = Vec::new();
    let mut checked = 0usize;

    for main_path in &sample_mains {
        let source = fs::read_to_string(main_path).unwrap_or_default();
        let Some(expected) = documented_expected_exit(&source) else {
            continue;
        };
        let name = sample_name(main_path);
        let build_dir = std::env::temp_dir()
            .join(format!("omega-sample-run-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&build_dir);

        match compile(CompileOptions {
            root_path: main_path.clone(),
            build_dir: Some(build_dir.clone()),
            target_name: None,
            write_output: true,
        }) {
            Err(error) => failures.push(format!("{name}: compile failed: {error:?}")),
            Ok(_) => {
                // stdin is closed (Stdio::null): a sample that reads input sees EOF
                // and must still reach its documented deterministic exit.
                match Command::new(build_dir.join(executable_name()))
                    .stdin(Stdio::null())
                    .output()
                {
                    Ok(output) => {
                        if output.status.code() != Some(expected) {
                            failures.push(format!(
                                "{name}: exit {:?}, expected {expected}\nstderr:\n{}",
                                output.status.code(),
                                String::from_utf8_lossy(&output.stderr)
                            ));
                        }
                        // A renderer can exit cleanly while drawing NOTHING (a broken
                        // carrier render). If the sample documents an expected output
                        // substring, assert the stdout actually contains it.
                        if let Some(expected_output) = documented_expected_output(&source) {
                            let stdout = String::from_utf8_lossy(&output.stdout);
                            if !stdout.contains(&expected_output) {
                                failures.push(format!(
                                    "{name}: stdout did not contain {expected_output:?} \
                                     (a renderer that exits cleanly but drew nothing); \
                                     {} bytes of stdout",
                                    output.stdout.len()
                                ));
                            }
                        }
                        checked += 1;
                    }
                    Err(error) => failures.push(format!("{name}: run failed: {error}")),
                }
            }
        }
        let _ = fs::remove_dir_all(&build_dir);
    }

    assert!(
        checked > 0,
        "expected at least one sample with a documented `Expected exit:` annotation"
    );
    assert!(
        failures.is_empty(),
        "{} samples ran with the wrong exit (a runtime miscompile that still \
         compiles):\n{}",
        failures.len(),
        failures.join("\n")
    );
}
