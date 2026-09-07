use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn omega(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .output()
        .expect("run omega")
}

fn omega_in(directory: &std::path::Path, arguments: &[&str]) -> Output {
    let executable = std::fs::canonicalize(env!("CARGO_BIN_EXE_omega"))
        .expect("resolve omega before changing child directory");
    Command::new(executable)
        .current_dir(directory)
        .args(arguments)
        .output()
        .expect("run omega")
}

fn temp_path(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "omega-package-quarantine-{name}-{}-{stamp}",
        std::process::id()
    ))
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("omega crate should live under omega-rust/omega")
        .to_path_buf()
}

#[test]
fn routed_production_entry_roots_pass_real_package_resolution() {
    for (label, relative) in [
        ("omega-product", "source/omega/main.omg"),
        ("parser-gate", "source/psi/gates/parser/main.omg"),
    ] {
        let source = repository_root().join(relative);
        let build_dir = temp_path(label);
        let source = source.to_string_lossy().into_owned();
        let build_dir_argument = build_dir.to_string_lossy().into_owned();
        let output = omega(&["--check", "--build-dir", &build_dir_argument, &source]);
        assert!(
            output.status.success(),
            "{relative} should pass real package resolution:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let _ = std::fs::remove_dir_all(build_dir);
    }
}

#[test]
fn package_command_words_do_not_reserve_ordinary_source_filenames() {
    let project = temp_path("install-source");
    std::fs::create_dir(&project).expect("create source project");
    for source_name in ["install.omg", "update.omg"] {
        std::fs::write(project.join(source_name), b"machine main() {}\n")
            .expect("write ordinary source");

        let output = omega_in(&project, &[source_name, "--check"]);

        assert!(
            output.status.success(),
            "ordinary source `{source_name}` should compile: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let _ = std::fs::remove_dir_all(project);
}

#[cfg(unix)]
#[test]
fn dependency_free_build_project_still_enters_reconciled_source_custody() {
    use std::os::unix::fs::symlink;

    let project = temp_path("zero-dependency-project");
    let outside = temp_path("zero-dependency-outside.omg");
    std::fs::create_dir(&project).expect("create dependency-free project");
    std::fs::write(
        project.join("build.omg"),
        b"machine build(builder: &mut Build) { builder.application(\"zero-dependency\"); }\n",
    )
    .expect("write project build root");
    std::fs::write(project.join("main.omg"), b"machine main() {}\n").expect("write project entry");
    std::fs::write(&outside, b"machine escaped() {}\n").expect("write outside source");
    symlink(&outside, project.join("escaped.omg")).expect("create escaping source link");

    let output = omega_in(&project, &["main.omg", "--check"]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "escaping source link was accepted"
    );
    assert!(
        stderr.contains("source symlink") && stderr.contains("outside package root"),
        "dependency-free build did not report reconciled source custody: {stderr}"
    );
    let _ = std::fs::remove_dir_all(project);
    let _ = std::fs::remove_file(outside);
}

#[test]
fn production_usage_does_not_advertise_manifest_or_receipt_inputs() {
    let output = omega(&[]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("manifest.json"), "stderr was: {stderr}");
    assert!(!stderr.contains("receipt.json"), "stderr was: {stderr}");
    assert!(!stderr.contains("lock assemble"), "stderr was: {stderr}");
    assert!(
        !stderr.contains("--package-root-policy"),
        "stderr was: {stderr}"
    );
}

#[test]
fn optimizer_rollback_cli_requires_exact_unique_names() {
    let unknown = omega(&[
        "--disable-optimization",
        "control_flow_cleanup",
        "missing.omg",
    ]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&unknown.stderr)
            .contains("unknown exact optimization rollback name `control_flow_cleanup`")
    );

    let duplicate = omega(&[
        "--disable-optimization",
        "ControlFlowCleanup",
        "--disable-optimization",
        "ControlFlowCleanup",
        "missing.omg",
    ]);
    assert_eq!(duplicate.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&duplicate.stderr)
            .contains("optimization rollback repeats `ControlFlowCleanup`")
    );

    let missing = omega(&["--disable-optimization"]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&missing.stderr)
            .contains("--disable-optimization requires one exact optimization name")
    );
}

#[test]
fn optimizer_rollback_cli_rejects_check_before_reading_source() {
    let output = omega(&[
        "--check",
        "--disable-optimization",
        "ControlFlowCleanup",
        "missing.omg",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("names stages not executed by Check"));
    assert!(stderr.contains("`ControlFlowCleanup`"));
    assert!(!stderr.contains("failed to read"));
}

fn accepted_claim_application() -> PathBuf {
    let project = temp_path("package-acceptance");
    std::fs::create_dir(&project).expect("create package application");
    std::fs::write(
        project.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.application("accepted-claim-app");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}
"#,
    )
    .expect("write application build declaration");
    std::fs::write(
        project.join("main.omg"),
        r#"pub proposition accepted(value: u64);

boundary machine trusted_value() -> u64
ensures accepted(result);

data Main { }
machine Main::main(&mut self) { }
"#,
    )
    .expect("write application source");
    std::fs::canonicalize(project).expect("resolve package application directory")
}

#[test]
fn package_native_cli_directs_missing_or_empty_lock_to_update() {
    let project = accepted_claim_application();
    let build_dir = temp_path("package-acceptance-build");
    let build_dir_argument = build_dir.to_string_lossy().into_owned();
    for empty_lock in [false, true] {
        if empty_lock {
            std::fs::write(project.join("omega.lock"), b"").expect("write empty lock");
        }
        let output = omega_in(
            &project,
            &[
                "--output-only",
                "--target",
                "linux_x86_64",
                "--build-dir",
                &build_dir_argument,
                "main.omg",
            ],
        );

        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{diagnostic}");
        assert!(diagnostic.contains("run omega update"), "{diagnostic}");
        assert!(
            !diagnostic.contains("--package-root-policy"),
            "{diagnostic}"
        );
        if empty_lock {
            assert!(
                std::fs::read(project.join("omega.lock"))
                    .unwrap()
                    .is_empty()
            );
        } else {
            assert!(!project.join("omega.lock").exists());
            for expected in ["omega-package-review 1", "baseline none", "end-review"] {
                assert!(
                    diagnostic.lines().any(|line| line == expected),
                    "{diagnostic}"
                );
            }
            assert!(
                diagnostic
                    .lines()
                    .any(|line| line.starts_with("candidate ")),
                "{diagnostic}"
            );
            let package = diagnostic
                .split("\npackage ")
                .find(|section| section.starts_with("\"accepted-claim-app\" "))
                .and_then(|section| section.split_once("\nend-package"))
                .map(|(package, _)| package)
                .unwrap_or_else(|| panic!("missing application policy section: {diagnostic}"));
            assert!(
                package.lines().any(|line| line == "change callable added"),
                "{package}"
            );
            assert!(
                package.lines().any(|line| {
                    line.starts_with("decision row ") && line.ends_with(" pending")
                }),
                "{package}"
            );
        }
    }
    let _ = std::fs::remove_dir_all(project);
    let _ = std::fs::remove_dir_all(build_dir);
}

#[test]
fn package_native_cli_reuses_update_acceptance_before_publication() {
    let project = accepted_claim_application();
    let build_dir = temp_path("accepted-package-native-build");
    let update = omega_in(
        &project,
        &["update", "--target", "linux_x86_64", "--offline"],
    );
    assert_eq!(
        update.status.code(),
        Some(3),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&update.stdout),
        String::from_utf8_lossy(&update.stderr)
    );
    assert!(!project.join("omega.lock").exists());
    let stdout = String::from_utf8_lossy(&update.stdout);
    let review_paths: Vec<_> = stdout
        .lines()
        .filter_map(|line| line.strip_prefix("review: "))
        .map(PathBuf::from)
        .collect();
    assert_eq!(review_paths.len(), 1, "{stdout}");
    let review_path = &review_paths[0];
    assert!(review_path.starts_with(project.join("build/package-manager")));
    let document = std::fs::read_to_string(review_path).expect("read generated package review");
    assert!(document.lines().any(|line| line.starts_with("decision ")));
    // Only the generated review's decision tokens are owner-editable.
    let accepted = document
        .lines()
        .map(|line| {
            if line.starts_with("decision ") {
                format!(
                    "{} accept\n",
                    line.strip_suffix(" pending")
                        .expect("fresh pending decision")
                )
            } else {
                format!("{line}\n")
            }
        })
        .collect::<String>();
    std::fs::write(review_path, accepted).expect("accept generated review decisions");
    let resumed = omega_in(&project, &["update", "--resume", "--offline"]);
    assert!(
        resumed.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&resumed.stdout),
        String::from_utf8_lossy(&resumed.stderr)
    );
    let accepted_lock = std::fs::read(project.join("omega.lock")).expect("read accepted lock");
    assert!(!accepted_lock.is_empty());
    let build_dir_argument = build_dir.to_string_lossy().into_owned();
    let native = omega_in(
        &project,
        &[
            "--output-only",
            "--offline",
            "--target",
            "linux_x86_64",
            "--build-dir",
            &build_dir_argument,
            "main.omg",
        ],
    );
    // TARGET-MATRICES in TASKS_OPTIMIZER.md owns the missing function-validation
    // evidence producer. Package acceptance must reach that independent gate,
    // not ask for another approval or weaken publication to make this test pass.
    assert_eq!(
        native.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&native.stdout),
        String::from_utf8_lossy(&native.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&native.stderr).trim(),
        "native publication requires compiler-function validation evidence"
    );
    assert!(!String::from_utf8_lossy(&native.stdout).contains("published native output"));
    assert_eq!(
        std::fs::read(project.join("omega.lock")).expect("read lock after native build"),
        accepted_lock,
        "native compilation changed project acceptance"
    );
    let _ = std::fs::remove_dir_all(project);
    let _ = std::fs::remove_dir_all(build_dir);
}

#[test]
fn obsolete_package_root_policy_is_an_unrecognized_option() {
    for arguments in [
        vec!["--package-root-policy"],
        vec!["--package-root-policy", "candidate.policy", "missing.omg"],
        vec![
            "--check",
            "--package-root-policy",
            "candidate.policy",
            "missing.omg",
        ],
    ] {
        let output = omega(&arguments);
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{diagnostic}");
        assert!(
            diagnostic.contains("unrecognized option `--package-root-policy`"),
            "{diagnostic}"
        );
    }
}

#[test]
fn source_audit_requires_an_explicit_supported_adapter() {
    let missing = omega(&["audit", "source", "https://example.invalid/package"]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("--kind <local|git>"));

    let unsupported = omega(&[
        "audit",
        "source",
        "--kind",
        "archive",
        "https://example.invalid/package.tar",
    ]);
    assert_eq!(unsupported.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&unsupported.stderr).contains("UnsupportedSourceAdapter"));
}

/// The primary documented route (`omega [--target <t>] <root.omg>`) reaches native
/// realization on the caller's thread, so it ran the deepest walks on whatever stack
/// the host gave `main` -- one mebibyte on Windows, where this aborted at exit 127
/// with zero artifacts written.
///
/// This drives a real sample on purpose. The synthetic project in
/// `package_native_cli_directs_missing_or_empty_lock_to_update` exercises the same
/// route and passes either way, because its machine is shallow enough to fit in a
/// mebibyte. Deeply nested source does not discriminate either: the parser's own
/// 1024-level guard is reached identically on both stack sizes. Only a real sample
/// goes deep enough, and this test takes about four minutes as a result.
#[test]
fn the_primary_native_route_survives_a_real_sample() {
    let source = repository_root().join("samples/cli/basics/cli_mvp/main.omg");
    let build_dir = temp_path("primary-native-route");
    let source = source.to_string_lossy().into_owned();
    let build_dir_argument = build_dir.to_string_lossy().into_owned();

    let output = omega(&["--build-dir", &build_dir_argument, &source]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stderr.contains("overflowed its stack"),
        "the primary route overflowed the host stack:{}{stderr}",
        "
"
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected the package acceptance diagnostic, stderr was:{}{stderr}",
        "
"
    );
    assert!(
        stderr.contains("run omega update"),
        "stderr was:{}{stderr}",
        "
"
    );
    let _ = std::fs::remove_dir_all(build_dir);
}

/// An unmatched argument used to fall through to the root path, so a misspelled
/// flag was taken as a source root and the directory containing it was walked as a
/// package. `omega --bogus` reported `source root exceeds identity entry limit of
/// 4096` from the repository root rather than naming the bad option.
#[test]
fn unrecognized_options_are_rejected_rather_than_taken_as_the_root() {
    let output = omega(&["--bogus"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(2),
        "stderr was:{}{stderr}",
        "
"
    );
    assert!(
        stderr.contains("unrecognized option `--bogus`"),
        "stderr was:{}{stderr}",
        "
"
    );
    assert!(
        !stderr.contains("identity entry limit"),
        "the option was still taken as a source root:{}{stderr}",
        "
"
    );
}
