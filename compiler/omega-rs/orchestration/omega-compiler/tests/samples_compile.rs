//! Every sample app under `samples/` must reach checked semantics.
//!
//! Samples otherwise have almost no compile coverage — only `cli_mvp` is built
//! by the canary suite and only the dungeon is parse-tested — so they silently
//! bit-rot against language changes. That is exactly what happened when
//! exact-arithmetic (decision 17) became a proof obligation: 22 of the 48
//! samples stopped compiling and nothing noticed. This harness is the guard:
//! one iterating test that checks every sample `main.omg` under `samples/` for
//! the default target and reports *all* broken samples at once, so a language
//! change that breaks a demo fails the suite the same day. This broad source-
//! compatibility sweep is entry-agnostic. Authored entry migration is checked
//! separately. The broad native oracle uses an authored host entry whenever one
//! exists and keeps its exact host-owned staging adapter only for unrooted
//! legacy samples (plus explicitly documented lowering gaps).
//!
//! Four guards:
//!  * `all_samples_reach_checked_trees` — every sample `main.omg` under
//!    `samples/` must reach checked semantics (catches staleness like the
//!    decision-17 break without inventing deployment entry policy).
//!  * `basics_samples_compile_from_authored_program_entry_bindings` — the
//!    migrated basics cohort selects `Main::main` for every hosted target and
//!    directly lowers every entry shape the production backend currently
//!    supports.
//!  * `algorithm_samples_compile_from_authored_program_entry_bindings` — the
//!    migrated algorithms cohort has the same exact selection and direct-
//!    lowering guarantees, with no legacy staging adapter.
//!  * `interpreter_samples_compile_from_authored_program_entry_bindings` — the
//!    migrated interpreter cohort likewise lowers directly for every hosted
//!    target without legacy staging.
//!  * `game_samples_compile_from_authored_program_entry_bindings` — the
//!    migrated games cohort has the same exact hosted-root guarantees.
//!  * `proof_samples_compile_from_authored_program_entry_bindings` — the five
//!    deployable proof samples do the same, while the two proof-only sources
//!    remain targetless checked fixtures.
//!  * `samples_with_documented_exit_run_correctly` — every sample whose comment
//!    states `Expected exit: N` is compiled, RUN, and its exit asserted. This
//!    catches runtime miscompiles that compile cleanly: stack_vm exited 71 vs 70
//!    for weeks because nothing ran the samples. Now a native miscompile in a
//!    demo fails the suite the same day. A sample may ALSO state
//!    `Expected output contains: <text>`, and its stdout is asserted to contain
//!    that text — exit code alone passes even when a RENDERER silently draws
//!    nothing (a broken carrier render), so the renderers assert a glyph they draw.

use omega_compiler::{CompileOptions, compile as compile_program, compile_to_checked};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ENTRY_STAGE: AtomicU64 = AtomicU64::new(1);
const SAMPLE_ENTRY: &str = "omega_sample_entry";
const HOSTED_SAMPLE_TARGETS: &[&str] = &["windows_x64", "linux_x64", "linux_arm64", "macos_arm64"];
const EXPLICIT_ENTRY_BASIC_SAMPLES: &[&str] = &[
    "brightness_control",
    "cli_mvp",
    "generic_counters",
    "multiplication_table",
    "nested_diagnostics",
    "number_guess",
    "print_number",
    "print_squares",
    "temperature_convert",
    "text_greeting",
    "unit_converter",
];
const DIRECT_ENTRY_NATIVE_BASIC_SAMPLES: &[&str] = &[
    "brightness_control",
    "cli_mvp",
    "generic_counters",
    "multiplication_table",
    "nested_diagnostics",
    "number_guess",
    "print_number",
    "print_squares",
    "temperature_convert",
    "text_greeting",
    "unit_converter",
];
const EXPLICIT_ENTRY_ALGORITHM_SAMPLES: &[&str] = &[
    "binary_search_viz",
    "bubble_sort",
    "dutch_flag",
    "horner_eval",
    "insertion_sort",
    "longest_run",
    "maze_flood",
    "sort_visualizer",
];
const EXPLICIT_ENTRY_INTERPRETER_SAMPLES: &[&str] = &[
    "calculator",
    "calculator_rpn",
    "rpn_calculator",
    "stack_calculator",
    "stack_vm",
    "token_interpreter",
];
const EXPLICIT_ENTRY_GAME_SAMPLES: &[&str] = &[
    "dice_histogram",
    "dice_roller",
    "score_tracker",
    "scoreboard",
    "tic_tac_toe",
    "turn_combat",
    "vending_machine",
];
const EXPLICIT_ENTRY_PROOF_SAMPLES: &[&str] = &[
    "bounded_counter",
    "clamp_sum",
    "leap_year",
    "shape_area",
    "shapes_area",
];

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
/// samples/gui/window_app, which waits for a human to close its window).
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
    let mut mains = Vec::new();
    collect_sample_mains(&samples_dir, &mut mains);
    mains.sort();
    assert!(
        !mains.is_empty(),
        "expected sample apps under {}",
        samples_dir.display()
    );
    mains
}

fn collect_sample_mains(directory: &Path, mains: &mut Vec<PathBuf>) {
    if directory.join("main.omg").is_file() {
        mains.push(directory.join("main.omg"));
        return;
    }

    let entries = fs::read_dir(directory).unwrap_or_else(|error| {
        panic!("failed to read directory {}: {error}", directory.display())
    });
    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("failed to read directory entry: {error}"))
            .path();
        if !path.is_dir() {
            continue;
        }
        if path
            .file_name()
            .is_some_and(|file_name| file_name == "build")
        {
            continue;
        }
        collect_sample_mains(&path, mains);
    }
}

fn sample_name(main_path: &Path) -> String {
    let samples_dir = repo_root().join("samples");
    let Some(parent) = main_path.parent() else {
        return "<unknown>".to_owned();
    };
    let relative = parent.strip_prefix(&samples_dir).unwrap_or(parent);
    let components: Vec<_> = relative
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect();
    if components.is_empty() {
        "<unknown>".to_owned()
    } else {
        components.join("__")
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under compiler/orchestration/omega-compiler")
        .to_path_buf()
}

fn compile_sample_runtime_entry(
    options: CompileOptions,
) -> Result<omega_compiler::CompileReport, Vec<psi_diagnostics::Diagnostic>> {
    let has_authored_entry = fs::read_to_string(
        options
            .root_path
            .parent()
            .expect("sample source has a project directory")
            .join("build.omg"),
    )
    .is_ok_and(|source| {
        source.contains(".roots.bind(")
            && source.contains(&format!("{}::ProgramEntry", host_root_owner()))
    });
    if has_authored_entry {
        return compile_program(CompileOptions {
            target_name: Some(host_target_name().to_owned()),
            ..options
        });
    }

    compile_staged_legacy_host_entry(options)
}

fn compile_staged_legacy_host_entry(
    options: CompileOptions,
) -> Result<omega_compiler::CompileReport, Vec<psi_diagnostics::Diagnostic>> {
    let ordinal = NEXT_ENTRY_STAGE.fetch_add(1, Ordering::Relaxed);
    let stage_dir = std::env::temp_dir().join(format!(
        "omega-sample-entry-stage-{}-{ordinal}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&stage_dir);
    if let Err(error) =
        stage_exact_host_entry_project(&options.root_path, &stage_dir, options.build_dir.as_deref())
    {
        let _ = fs::remove_dir_all(&stage_dir);
        return Err(vec![psi_diagnostics::Diagnostic::error(format!(
            "failed to stage legacy sample entry: {error}"
        ))]);
    }

    let result = compile_program(CompileOptions {
        root_path: stage_dir.join(
            options
                .root_path
                .file_name()
                .expect("sample source has a file name"),
        ),
        build_dir: options.build_dir,
        target_name: Some(host_target_name().to_owned()),
        write_output: options.write_output,
    });
    let _ = fs::remove_dir_all(&stage_dir);
    result
}

fn stage_exact_host_entry_project(
    main_path: &Path,
    destination: &Path,
    excluded: Option<&Path>,
) -> std::io::Result<()> {
    copy_project_tree(
        main_path
            .parent()
            .expect("sample source has a project directory"),
        destination,
        excluded,
    )?;
    write_exact_host_entry_adapter(main_path, destination)?;
    write_exact_host_build(destination)
}

fn copy_project_tree(
    source: &Path,
    destination: &Path,
    excluded: Option<&Path>,
) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        if excluded.is_some_and(|excluded| path == excluded)
            || matches!(entry.file_name().to_str(), Some("build" | "target"))
        {
            continue;
        }
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_project_tree(&path, &target, excluded)?;
        } else {
            fs::copy(path, target)?;
        }
    }
    Ok(())
}

fn write_exact_host_entry_adapter(main_path: &Path, destination: &Path) -> std::io::Result<()> {
    let main_source = fs::read_to_string(main_path)?;
    let signature_start = main_source.find("machine Main::main(").ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "sample source has no Main::main machine",
        )
    })?;
    let signature_end = main_source[signature_start..]
        .find('{')
        .map(|offset| signature_start + offset)
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "sample Main::main has no body",
            )
        })?;
    let call = if main_source[signature_start..signature_end].contains("->") {
        "_ = self.main();"
    } else {
        "self.main();"
    };
    fs::write(
        destination.join(
            main_path
                .file_name()
                .expect("sample source has a file name"),
        ),
        format!("{main_source}\n\nmachine Main::{SAMPLE_ENTRY}(&mut self) {{\n    {call}\n}}\n"),
    )
}

fn write_exact_host_build(project: &Path) -> std::io::Result<()> {
    let path = project.join("build.omg");
    let mut source = fs::read_to_string(&path).unwrap_or_default();
    let target = host_target_name();
    if !source.contains(&format!("target {target}")) {
        source.push_str(&format!("\n\ntarget {target} {{\n}}\n"));
    }
    let binding = format!(
        "{}.roots.bind({}::ProgramEntry, Main::{SAMPLE_ENTRY});",
        build_parameter_name(&source).unwrap_or("b"),
        host_root_owner(),
    );
    source = replace_host_program_entry_binding(&source, &binding);
    if !source.contains(&binding) {
        if let Some(open_brace) = build_machine_open_brace(&source) {
            source.insert_str(open_brace + 1, &format!("\n    {binding}"));
        } else {
            source.push_str(&format!(
                "\n\nmachine build(b: &mut Build) {{\n    {binding}\n}}\n"
            ));
        }
    }
    fs::write(path, source)
}

fn replace_host_program_entry_binding(source: &str, binding: &str) -> String {
    let marker = format!("{}::ProgramEntry", host_root_owner());
    let mut replaced = false;
    let mut lines = Vec::new();
    for line in source.lines() {
        if line.contains(".roots.bind(") && line.contains(&marker) {
            if !replaced {
                let indent = &line[..line.len() - line.trim_start().len()];
                lines.push(format!("{indent}{binding}"));
                replaced = true;
            }
        } else {
            lines.push(line.to_owned());
        }
    }
    let mut rewritten = lines.join("\n");
    if source.ends_with('\n') {
        rewritten.push('\n');
    }
    rewritten
}

fn build_machine_open_brace(source: &str) -> Option<usize> {
    let start = source.find("machine build(")?;
    source[start..].find('{').map(|offset| start + offset)
}

fn build_parameter_name(source: &str) -> Option<&str> {
    let start = source.find("machine build(")?;
    let signature_end = source[start..].find('{').map(|offset| start + offset)?;
    let signature = &source[start..signature_end];
    let type_marker = signature.rfind(": &mut Build")?;
    let prefix = signature[..type_marker].trim_end();
    let name_start = prefix
        .rfind(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .map_or(0, |index| index + 1);
    let name = &prefix[name_start..];
    (!name.is_empty()).then_some(name)
}

fn host_target_name() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "macos_arm64"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "linux_arm64"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "linux_x64"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "windows_x64"
    } else {
        panic!("unsupported host profile for native sample execution")
    }
}

fn host_root_owner() -> &'static str {
    match host_target_name() {
        "macos_arm64" => "macos_arm64",
        "linux_arm64" => "linux_arm64",
        "linux_x64" => "linux_x86_64",
        "windows_x64" => "windows_x86_64",
        _ => unreachable!("host_target_name returns one hosted target"),
    }
}

#[test]
fn basics_samples_compile_from_authored_program_entry_bindings() {
    for sample in EXPLICIT_ENTRY_BASIC_SAMPLES {
        let main_path = repo_root()
            .join("samples/cli/basics")
            .join(sample)
            .join("main.omg");
        for target in HOSTED_SAMPLE_TARGETS {
            let checked =
                compile_to_checked(&main_path, Some(target)).unwrap_or_else(|diagnostics| {
                    panic!(
                        "basic sample {sample} should select its authored {target} entry: \
                     {diagnostics:#?}"
                    )
                });
            assert_eq!(
                checked.selected_program_entry_machine(),
                Some("Main::main"),
                "basic sample {sample} must select its authored Main::main binding for {target}"
            );
        }
        let checked = compile_to_checked(&main_path, None).unwrap_or_else(|diagnostics| {
            panic!(
                "basic sample {sample} should remain entry-agnostic when checked: {diagnostics:#?}"
            )
        });
        assert_eq!(
            checked.selected_program_entry_machine(),
            None,
            "checked-only basic sample {sample} must not select a storage root"
        );
    }

    for sample in DIRECT_ENTRY_NATIVE_BASIC_SAMPLES {
        let main_path = repo_root()
            .join("samples/cli/basics")
            .join(sample)
            .join("main.omg");
        let build_dir = std::env::temp_dir().join(format!(
            "omega-authored-basic-entry-{sample}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);
        compile_program(CompileOptions {
            root_path: main_path,
            build_dir: Some(build_dir.clone()),
            target_name: Some(host_target_name().to_owned()),
            write_output: false,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "basic sample {sample} should compile directly without a staged entry: \
                 {diagnostics:#?}"
            )
        });
        let _ = fs::remove_dir_all(build_dir);
    }
}

#[test]
fn algorithm_samples_compile_from_authored_program_entry_bindings() {
    for sample in EXPLICIT_ENTRY_ALGORITHM_SAMPLES {
        let main_path = repo_root()
            .join("samples/cli/algorithms")
            .join(sample)
            .join("main.omg");
        for target in HOSTED_SAMPLE_TARGETS {
            let checked =
                compile_to_checked(&main_path, Some(target)).unwrap_or_else(|diagnostics| {
                    panic!(
                        "algorithm sample {sample} should select its authored {target} entry: \
                         {diagnostics:#?}"
                    )
                });
            assert_eq!(
                checked.selected_program_entry_machine(),
                Some("Main::main"),
                "algorithm sample {sample} must select its authored Main::main binding for \
                 {target}"
            );

            let build_dir = std::env::temp_dir().join(format!(
                "omega-authored-algorithm-entry-{sample}-{target}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&build_dir);
            compile_program(CompileOptions {
                root_path: main_path.clone(),
                build_dir: Some(build_dir.clone()),
                target_name: Some((*target).to_owned()),
                write_output: false,
            })
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "algorithm sample {sample} should lower directly for {target} without a \
                     staged entry: {diagnostics:#?}"
                )
            });
            let _ = fs::remove_dir_all(build_dir);
        }
        let checked = compile_to_checked(&main_path, None).unwrap_or_else(|diagnostics| {
            panic!(
                "algorithm sample {sample} should remain entry-agnostic when checked: \
                 {diagnostics:#?}"
            )
        });
        assert_eq!(
            checked.selected_program_entry_machine(),
            None,
            "checked-only algorithm sample {sample} must not select a storage root"
        );
    }
}

#[test]
fn interpreter_samples_compile_from_authored_program_entry_bindings() {
    for sample in EXPLICIT_ENTRY_INTERPRETER_SAMPLES {
        let main_path = repo_root()
            .join("samples/cli/interpreters")
            .join(sample)
            .join("main.omg");
        for target in HOSTED_SAMPLE_TARGETS {
            let checked =
                compile_to_checked(&main_path, Some(target)).unwrap_or_else(|diagnostics| {
                    panic!(
                        "interpreter sample {sample} should select its authored {target} entry: \
                         {diagnostics:#?}"
                    )
                });
            assert_eq!(
                checked.selected_program_entry_machine(),
                Some("Main::main"),
                "interpreter sample {sample} must select its authored Main::main binding for \
                 {target}"
            );

            let build_dir = std::env::temp_dir().join(format!(
                "omega-authored-interpreter-entry-{sample}-{target}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&build_dir);
            compile_program(CompileOptions {
                root_path: main_path.clone(),
                build_dir: Some(build_dir.clone()),
                target_name: Some((*target).to_owned()),
                write_output: false,
            })
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "interpreter sample {sample} should lower directly for {target} without a \
                     staged entry: {diagnostics:#?}"
                )
            });
            let _ = fs::remove_dir_all(build_dir);
        }
        let checked = compile_to_checked(&main_path, None).unwrap_or_else(|diagnostics| {
            panic!(
                "interpreter sample {sample} should remain entry-agnostic when checked: \
                 {diagnostics:#?}"
            )
        });
        assert_eq!(
            checked.selected_program_entry_machine(),
            None,
            "checked-only interpreter sample {sample} must not select a storage root"
        );
    }
}

#[test]
fn game_samples_compile_from_authored_program_entry_bindings() {
    for sample in EXPLICIT_ENTRY_GAME_SAMPLES {
        let main_path = repo_root()
            .join("samples/cli/games")
            .join(sample)
            .join("main.omg");
        for target in HOSTED_SAMPLE_TARGETS {
            let checked =
                compile_to_checked(&main_path, Some(target)).unwrap_or_else(|diagnostics| {
                    panic!(
                        "game sample {sample} should select its authored {target} entry: \
                         {diagnostics:#?}"
                    )
                });
            assert_eq!(
                checked.selected_program_entry_machine(),
                Some("Main::main"),
                "game sample {sample} must select its authored Main::main binding for {target}"
            );

            let build_dir = std::env::temp_dir().join(format!(
                "omega-authored-game-entry-{sample}-{target}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&build_dir);
            compile_program(CompileOptions {
                root_path: main_path.clone(),
                build_dir: Some(build_dir.clone()),
                target_name: Some((*target).to_owned()),
                write_output: false,
            })
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "game sample {sample} should lower directly for {target} without a staged \
                     entry: {diagnostics:#?}"
                )
            });
            let _ = fs::remove_dir_all(build_dir);
        }
        let checked = compile_to_checked(&main_path, None).unwrap_or_else(|diagnostics| {
            panic!(
                "game sample {sample} should remain entry-agnostic when checked: \
                 {diagnostics:#?}"
            )
        });
        assert_eq!(
            checked.selected_program_entry_machine(),
            None,
            "checked-only game sample {sample} must not select a storage root"
        );
    }
}

#[test]
fn temperature_sample_retains_exact_float_operator_evidence() {
    let main_path = repo_root().join("samples/cli/basics/temperature_convert/main.omg");
    let checked = compile_to_checked(&main_path, Some(host_target_name()))
        .expect("temperature sample should reach checked trees");
    let uses = checked
        .facts
        .operators
        .uses
        .iter()
        .map(|(_, operator_use)| operator_use)
        .filter(|operator_use| {
            matches!(
                operator_use.origin,
                psi_checked_trees::CheckedValueOrigin::StateStatement {
                    statement_index: 2,
                    role: psi_checked_trees::CheckedValueStatementRole::AssignmentValue,
                    ..
                }
            ) && matches!(
                checked
                    .typed
                    .expression_table
                    .expression(operator_use.expression),
                psi_typed_trees::expression::ExpressionNode::Binary(binary)
                    if matches!(binary.operator,
                        psi_typed_trees::expression::BinaryOperator::Add
                            | psi_typed_trees::expression::BinaryOperator::Multiply)
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        uses.len(),
        2,
        "the nested float assignment must retain its multiply and add"
    );
    assert!(
        uses.iter()
            .all(|operator_use| operator_use.provider_plan_identity != 0),
        "both nested operations must retain their exact selected ProviderPlan"
    );
}

#[test]
fn proof_samples_compile_from_authored_program_entry_bindings() {
    for sample in EXPLICIT_ENTRY_PROOF_SAMPLES {
        let main_path = repo_root()
            .join("samples/cli/proofs")
            .join(sample)
            .join("main.omg");
        for target in HOSTED_SAMPLE_TARGETS {
            let checked =
                compile_to_checked(&main_path, Some(target)).unwrap_or_else(|diagnostics| {
                    panic!(
                        "proof sample {sample} should select its authored {target} entry: \
                         {diagnostics:#?}"
                    )
                });
            assert_eq!(
                checked.selected_program_entry_machine(),
                Some("Main::main"),
                "proof sample {sample} must select its authored Main::main binding for {target}"
            );
        }
        let checked = compile_to_checked(&main_path, None).unwrap_or_else(|diagnostics| {
            panic!(
                "proof sample {sample} should remain entry-agnostic when checked: \
                 {diagnostics:#?}"
            )
        });
        assert_eq!(
            checked.selected_program_entry_machine(),
            None,
            "checked-only proof sample {sample} must not select a storage root"
        );
    }

    let mut lowering_failures = Vec::new();
    for sample in EXPLICIT_ENTRY_PROOF_SAMPLES {
        let main_path = repo_root()
            .join("samples/cli/proofs")
            .join(sample)
            .join("main.omg");
        let build_dir = std::env::temp_dir().join(format!(
            "omega-authored-proof-entry-{sample}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);
        if let Err(diagnostics) = compile_program(CompileOptions {
            root_path: main_path,
            build_dir: Some(build_dir.clone()),
            target_name: Some(host_target_name().to_owned()),
            write_output: false,
        }) {
            lowering_failures.push(format!("{sample}: {diagnostics:#?}"));
        }
        let _ = fs::remove_dir_all(build_dir);
    }
    assert!(
        lowering_failures.is_empty(),
        "{} deployable proof samples failed direct authored-entry lowering:\n{}",
        lowering_failures.len(),
        lowering_failures.join("\n")
    );
}

#[test]
fn all_samples_reach_checked_trees() {
    let sample_mains = sample_mains();
    let mut failures: Vec<String> = Vec::new();
    for main_path in &sample_mains {
        let name = sample_name(main_path);

        // Target-shaped samples check with their EXPLICIT cross target
        // (the registered `uefi_x64`, 2026-07-11v) -- the sample IS the
        // EFI image; host-target checking cannot satisfy it. Mirrors
        // the canary suite's CROSS_TARGET_PASS_CANARIES.
        let target_name = if name.contains("uefi_hello") {
            Some("uefi_x64".to_owned())
        } else {
            None
        };
        let result = compile_to_checked(main_path, target_name.as_deref());
        if let Err(error) = result {
            failures.push(format!("{name}: {error:?}"));
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} samples failed to reach checked trees (run the sample's main.omg through \
         omega-cli for the full diagnostic):\n{}",
        failures.len(),
        sample_mains.len(),
        failures.join("\n")
    );
}

#[test]
fn named_integer_conversion_samples_reach_checked_trees() {
    for relative in [
        "cli/probes/width_mixer",
        "cli/collections/array_sum",
        "cli/text/format_number",
        "cli/basics/print_number",
        "cli/basics/multiplication_table",
        "cli/arithmetic/prime_sieve",
        "cli/algorithms/maze_flood",
        "gui/image_viewer",
        "cli/games/dungeon_crawler_cli",
        "cli/games/dice_histogram",
        "cli/collections/heat_grid",
        "cli/text/parse_int",
        "cli/text/parse_number",
        "cli/text/substring_search",
        "cli/interpreters/calculator",
        "cli/systems/descriptor_walk",
        "cli/simulation/calendar",
        "cli/text/string_hash",
        "cli/arithmetic/factorial_loop",
        "cli/arithmetic/popcount",
        "cli/arithmetic/utf8_byte_class",
        "cli/basics/print_squares",
    ] {
        let main_path = repo_root().join("samples").join(relative).join("main.omg");
        compile_to_checked(&main_path, None).unwrap_or_else(|diagnostics| {
            panic!(
                "named integer-conversion sample {relative} should reach checked trees: \
                 {diagnostics:#?}"
            )
        });
    }
}

#[test]
fn samples_with_documented_exit_run_correctly() {
    let sample_mains = sample_mains();
    let mut failures: Vec<String> = Vec::new();
    let mut checked = 0usize;
    let filter = std::env::var("OMEGA_SAMPLE_RUNTIME_FILTER").ok();

    for main_path in &sample_mains {
        let source = fs::read_to_string(main_path).unwrap_or_default();
        let Some(expected) = documented_expected_exit(&source) else {
            continue;
        };
        let name = sample_name(main_path);
        if !filter.as_deref().is_none_or(|filter| {
            filter
                .split(',')
                .map(str::trim)
                .any(|candidate| !candidate.is_empty() && name.contains(candidate))
        }) {
            continue;
        }
        let build_dir =
            std::env::temp_dir().join(format!("omega-sample-run-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&build_dir);

        match compile_sample_runtime_entry(CompileOptions {
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
        "expected at least one selected sample with a documented `Expected exit:` annotation"
    );
    assert!(
        failures.is_empty(),
        "{} samples ran with the wrong exit (a runtime miscompile that still \
         compiles):\n{}",
        failures.len(),
        failures.join("\n")
    );
}
