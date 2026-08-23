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
//! separately. The native oracle executes only samples with an authored entry
//! for the host target. Target-shaped samples without one remain checked-only;
//! the harness never manufactures a machine or root binding for them.
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
//!  * `arithmetic_samples_compile_from_authored_program_entry_bindings` — the
//!    complete arithmetic cohort has the same exact hosted-root guarantees.
//!  * `system_samples_compile_from_authored_program_entry_bindings` — the
//!    complete systems cohort lowers from exact authored roots.
//!  * `probe_samples_compile_from_authored_program_entry_bindings` — executable
//!    regression probes name their exact hosted roots; the deliberate trapping
//!    fixture remains checked-only.
//!  * `stdin_samples_compile_from_authored_program_entry_bindings` — all stdin
//!    samples lower from exact authored roots on every hosted target.
//!  * `gui_samples_compile_from_authored_program_entry_bindings` — all GUI
//!    samples retain GUI subsystem policy while selecting exact Windows and
//!    macOS roots; Linux remains checked-only pending native GUI lowering.
//!  * `interpreter_samples_compile_from_authored_program_entry_bindings` — the
//!    migrated interpreter cohort likewise lowers directly for every hosted
//!    target without legacy staging.
//!  * `game_samples_compile_from_authored_program_entry_bindings` — the
//!    migrated games cohort has the same exact hosted-root guarantees.
//!  * `text_samples_compile_from_authored_program_entry_bindings` — the
//!    migrated text cohort has the same exact hosted-root guarantees.
//!  * `rendering_samples_compile_from_authored_program_entry_bindings` — the
//!    migrated rendering cohort has the same exact hosted-root guarantees.
//!  * `collection_samples_compile_from_authored_program_entry_bindings` — the
//!    complete collection cohort has the same guarantees, including recursive
//!    slice-value folds with cast terminals.
//!  * `simulation_samples_compile_from_authored_program_entry_bindings` — the
//!    deployable simulation cohort has the same exact hosted-root guarantees.
//!  * `proof_samples_compile_from_authored_program_entry_bindings` — the five
//!    deployable proof samples do the same, while the two proof-only sources
//!    remain targetless checked fixtures.
//!  * `sample_entry_exceptions_are_explicit_and_non_runnable` — the complete
//!    rootless corpus is the intentional proof/trap/firmware set, and any
//!    host-unavailable runtime sample is an explicit target-shaped exception.
//!  * `samples_with_documented_exit_run_correctly` — every sample whose comment
//!    states `Expected exit: N` and has an authored host root is compiled, RUN,
//!    and its exit asserted. This
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

const HOSTED_SAMPLE_TARGETS: &[&str] = &["windows_x64", "linux_x64", "linux_arm64", "macos_arm64"];
const GUI_SAMPLE_TARGETS: &[&str] = &["windows_x64", "macos_arm64"];
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
const EXPLICIT_ENTRY_ARITHMETIC_SAMPLES: &[&str] = &[
    "bit_shift",
    "collatz_sequence",
    "digital_root",
    "dot_product",
    "dual_float",
    "euclid_gcd",
    "factorial_loop",
    "fibonacci_golden",
    "led_mixer",
    "modular_exponentiation",
    "popcount",
    "prime_counter",
    "prime_sieve",
    "recursive_sum",
    "reverse_sum",
    "sensor_min_max",
    "smallest_prime_factor",
    "stats_compute",
    "utf8_byte_class",
    "vector_distance",
    "xorshift_prng",
];
const EXPLICIT_ENTRY_SYSTEM_SAMPLES: &[&str] = &[
    "account_ledger",
    "atomics_cross",
    "bank_ledger",
    "descriptor_walk",
    "elapsed_timer",
    "event_log",
    "file_journal",
    "file_permissions",
    "framed_payload",
    "logger",
    "note_vault",
    "status_report",
    "task_runner",
    "vending_machine",
    "wire_protocol",
];
const EXPLICIT_ENTRY_PROBE_SAMPLES: &[&str] = &[
    "alarm_probe",
    "alarm_probe2",
    "array_index_from_call",
    "direction_command",
    "dual_accumulator_recursion",
    "multi_value_calls",
    "nested_case_payload",
    "nested_counters",
    "self_mutation_between_calls",
    "value_call_in_expr",
    "width_mixer",
];
const EXPLICIT_ENTRY_STDIN_SAMPLES: &[&str] = &["stdin_checksum", "stdin_rot1", "stdin_upper"];
const EXPLICIT_ENTRY_GUI_SAMPLES: &[&str] = &[
    "image_viewer",
    "window_app",
    "window_demo",
    "windowed_calculator",
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
    "dungeon_crawler_cli",
    "score_tracker",
    "scoreboard",
    "tic_tac_toe",
    "turn_combat",
    "vending_machine",
];
const EXPLICIT_ENTRY_TEXT_SAMPLES: &[&str] = &[
    "caesar_cipher",
    "fletcher_checksum",
    "format_number",
    "luhn_checksum",
    "parse_int",
    "parse_number",
    "roman_numeral",
    "string_catalog",
    "string_hash",
    "substring_search",
    "text_padding",
];
const EXPLICIT_ENTRY_RENDERING_SAMPLES: &[&str] = &[
    "bouncing_ball",
    "bouncing_ball_2d",
    "bouncing_console",
    "bouncing_particles",
    "dungeon_render",
    "histogram",
    "mandelbrot",
    "mandelbrot_zoom",
    "pixel_canvas",
    "ripple_field",
    "tick_marquee",
];
const EXPLICIT_ENTRY_COLLECTION_SAMPLES: &[&str] = &[
    "array_max",
    "array_sum",
    "bitset",
    "bitset_sieve",
    "entity_list",
    "generic_ring_buffer",
    "heat_grid",
    "inventory_lookup",
    "inventory_system",
    "matrix_multiply",
    "slice_maximum",
    "slice_accum_probe",
    "subslice_sum",
];
const EXPLICIT_ENTRY_SIMULATION_SAMPLES: &[&str] = &[
    "alarm_scheduler",
    "calendar",
    "cellular_automaton",
    "elevator",
    "game_of_life",
    "game_of_life_glider",
    "grid_walk",
    "langtons_ant",
    "particle_sim",
    "random_walk",
    "stopwatch",
    "traffic_light",
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
        .nth(6)
        .expect("compiler crate should live under bootstrap/onramps/omega-rust/omega/orchestration/omega-compiler")
        .to_path_buf()
}

fn authored_program_entry(main_path: &Path, root_owner: Option<&str>) -> bool {
    let Some(project) = main_path.parent() else {
        return false;
    };
    fs::read_to_string(project.join("build.omg")).is_ok_and(|source| {
        source.lines().any(|line| {
            line.contains(".roots.bind(")
                && root_owner.is_none_or(|owner| line.contains(&format!("{owner}::ProgramEntry")))
        })
    })
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

fn assert_authored_entry_cohort(category: &str, samples: &[&str]) {
    assert_authored_entry_samples(
        &repo_root().join("samples/cli").join(category),
        category,
        samples,
        HOSTED_SAMPLE_TARGETS,
        true,
    );
}

fn assert_authored_entry_samples(
    base: &Path,
    cohort: &str,
    samples: &[&str],
    targets: &[&str],
    check_entry_agnostic: bool,
) {
    let mut failures = Vec::new();
    for sample in samples {
        let main_path = base.join(sample).join("main.omg");
        for target in targets {
            let checked = match compile_to_checked(&main_path, Some(target)) {
                Ok(checked) => checked,
                Err(diagnostics) => {
                    failures.push(format!(
                        "{sample}/{target}: authored-entry selection failed: {diagnostics:#?}"
                    ));
                    continue;
                }
            };
            if checked.selected_program_entry_machine() != Some("Main::main") {
                failures.push(format!(
                    "{sample}/{target}: selected {:?}, expected Main::main",
                    checked.selected_program_entry_machine()
                ));
                continue;
            }

            let build_dir = std::env::temp_dir().join(format!(
                "omega-authored-{cohort}-entry-{sample}-{target}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&build_dir);
            if let Err(diagnostics) = compile_program(CompileOptions {
                root_path: main_path.clone(),
                build_dir: Some(build_dir.clone()),
                target_name: Some((*target).to_owned()),
                write_output: false,
            }) {
                failures.push(format!(
                    "{sample}/{target}: direct authored-entry lowering failed: {diagnostics:#?}"
                ));
            }
            let _ = fs::remove_dir_all(build_dir);
        }
        if check_entry_agnostic {
            match compile_to_checked(&main_path, None) {
                Ok(checked) if checked.selected_program_entry_machine().is_none() => {}
                Ok(checked) => failures.push(format!(
                    "{sample}/checked-only: unexpectedly selected {:?}",
                    checked.selected_program_entry_machine()
                )),
                Err(diagnostics) => failures.push(format!(
                    "{sample}/checked-only: compilation failed: {diagnostics:#?}"
                )),
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} {cohort} authored-entry checks failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
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
    assert_authored_entry_cohort("algorithms", EXPLICIT_ENTRY_ALGORITHM_SAMPLES);
}

#[test]
fn arithmetic_samples_compile_from_authored_program_entry_bindings() {
    assert_authored_entry_cohort("arithmetic", EXPLICIT_ENTRY_ARITHMETIC_SAMPLES);
}

#[test]
fn system_samples_compile_from_authored_program_entry_bindings() {
    assert_authored_entry_cohort("systems", EXPLICIT_ENTRY_SYSTEM_SAMPLES);
}

#[test]
fn probe_samples_compile_from_authored_program_entry_bindings() {
    assert_authored_entry_cohort("probes", EXPLICIT_ENTRY_PROBE_SAMPLES);

    let main_path = repo_root().join("samples/cli/probes/trapping_probe/main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("the deliberate trapping probe should remain a checked-only fixture");
    assert_eq!(
        checked.selected_program_entry_machine(),
        None,
        "the trapping probe must not acquire a deployable storage root"
    );
}

#[test]
fn stdin_samples_compile_from_authored_program_entry_bindings() {
    assert_authored_entry_samples(
        &repo_root().join("samples"),
        "stdin",
        EXPLICIT_ENTRY_STDIN_SAMPLES,
        HOSTED_SAMPLE_TARGETS,
        true,
    );
}

#[test]
fn gui_samples_compile_from_authored_program_entry_bindings() {
    assert_authored_entry_samples(
        &repo_root().join("samples/gui"),
        "gui",
        EXPLICIT_ENTRY_GUI_SAMPLES,
        GUI_SAMPLE_TARGETS,
        // Targetless GUI checking is already covered by `all_samples_reach_checked_trees`;
        // repeating it here retains every unfiltered provider candidate and is needlessly slow.
        false,
    );
}

#[test]
fn interpreter_samples_compile_from_authored_program_entry_bindings() {
    assert_authored_entry_cohort("interpreters", EXPLICIT_ENTRY_INTERPRETER_SAMPLES);
}

#[test]
fn game_samples_compile_from_authored_program_entry_bindings() {
    assert_authored_entry_cohort("games", EXPLICIT_ENTRY_GAME_SAMPLES);
}

#[test]
fn text_samples_compile_from_authored_program_entry_bindings() {
    assert_authored_entry_cohort("text", EXPLICIT_ENTRY_TEXT_SAMPLES);
}

#[test]
fn rendering_samples_compile_from_authored_program_entry_bindings() {
    assert_authored_entry_cohort("rendering", EXPLICIT_ENTRY_RENDERING_SAMPLES);
}

#[test]
fn collection_samples_compile_from_authored_program_entry_bindings() {
    assert_authored_entry_cohort("collections", EXPLICIT_ENTRY_COLLECTION_SAMPLES);
}

#[test]
fn simulation_samples_compile_from_authored_program_entry_bindings() {
    assert_authored_entry_cohort("simulation", EXPLICIT_ENTRY_SIMULATION_SAMPLES);
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
fn sample_entry_exceptions_are_explicit_and_non_runnable() {
    let mains = sample_mains();
    let mut rootless = Vec::new();
    let mut host_unavailable_runtime = Vec::new();

    for main_path in &mains {
        let name = sample_name(main_path);
        let source = fs::read_to_string(main_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", main_path.display()));
        if !authored_program_entry(main_path, None) {
            assert_eq!(
                documented_expected_exit(&source),
                None,
                "rootless sample {name} must remain non-runnable"
            );
            rootless.push(name.clone());
        }
        if documented_expected_exit(&source).is_some()
            && !authored_program_entry(main_path, Some(host_root_owner()))
        {
            host_unavailable_runtime.push(name);
        }
    }

    assert_eq!(
        rootless,
        [
            "cli__probes__trapping_probe",
            "cli__proofs__math_proofs",
            "cli__proofs__structural_proofs",
            "uefi__uefi_hello",
        ],
        "only the deliberate trap, proof-only fixtures, and Q2-blocked firmware may lack an authored root"
    );

    let expected_host_unavailable = if host_target_name().starts_with("linux_") {
        &["gui__window_demo"][..]
    } else {
        &[]
    };
    assert_eq!(
        host_unavailable_runtime, expected_host_unavailable,
        "a documented runtime sample without an authored host root must be an explicit target-shaped exception"
    );
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
        if !authored_program_entry(main_path, Some(host_root_owner())) {
            // A target-shaped sample can document runtime behavior for its
            // supported targets while remaining checked-only on this host.
            // The corpus guard above owns the complete exception list.
            continue;
        }
        if !filter.as_deref().is_none_or(|filter| {
            filter.split(',').map(str::trim).any(|candidate| {
                candidate
                    .strip_prefix('=')
                    .is_some_and(|exact| name == exact)
                    || (!candidate.is_empty()
                        && !candidate.starts_with('=')
                        && name.contains(candidate))
            })
        }) {
            continue;
        }
        let build_dir =
            std::env::temp_dir().join(format!("omega-sample-run-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&build_dir);

        match compile_program(CompileOptions {
            root_path: main_path.clone(),
            build_dir: Some(build_dir.clone()),
            target_name: Some(host_target_name().to_owned()),
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
        failures.is_empty(),
        "{} samples ran with the wrong exit (a runtime miscompile that still \
         compiles):\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(
        checked > 0,
        "expected at least one selected sample with a documented `Expected exit:` annotation"
    );
}
