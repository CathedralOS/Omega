//! Differential oracle test: over every RUN canary (a `canaries/pass/**` program the
//! canary suite compiles, executes, and asserts a known exit code for), run the reference
//! interpreter and -- when it SUPPORTS the program -- compile + run the NATIVE binary and
//! assert their exit code and stdout MATCH.
//!
//! Framing: these are PASSING run canaries, so the native binary is correct-by-definition
//! (the suite asserts its exact exit code). Therefore any `interpret() != native` here is
//! an INTERPRETER bug, and the test fails so it gets fixed. When the interpreter returns an
//! error (unsupported construct) the canary is SKIPPED -- a missing feature is not a
//! mismatch. As the interpreter's coverage grows, more canaries leave the skip bucket and
//! must agree with native.
//!
//! The set below mirrors how `omega-compiler/tests/canary_suite.rs` runs canaries: only the
//! programs it actually executes-and-checks an exit code for (no compile-only or
//! visualization canaries, whose native exit code is undefined). The companion expected
//! code documents the suite's own assertion and lets us sanity-check native against it.

use omega_compiler::{compile, compile_to_checked, CompileOptions};
use omega_interpreter::interpret;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The RUN canaries: `(relative path under canaries/pass, exit code the suite asserts)`.
/// Extracted from `canary_suite.rs` (every test that runs `executable_name()` and asserts
/// `output.status.code() == Some(N)`).
const RUN_CANARIES: &[(&str, i32)] = &[
    ("arithmetic/runtime_chained_field_mutation_exit", 70),
    ("arithmetic/runtime_comparison_guard_signedness_exit", 70),
    ("arithmetic/runtime_comparison_value_signedness_exit", 70),
    ("arithmetic/runtime_copy_then_read_exit", 70),
    ("arithmetic/runtime_i64_full_width_exit", 70),
    ("arithmetic/runtime_min_max_signedness_exit", 70),
    ("arithmetic/runtime_signed_division_exit", 70),
    ("arithmetic/runtime_unsigned_division_exit", 70),
    ("calls/runtime_alias_indexed_read_through_transition_exit", 70),
    ("calls/runtime_alias_write_through_guarded_transition_exit", 70),
    ("calls/runtime_call_in_inlined_substate_exit", 70),
    ("calls/runtime_call_result_through_reference_field_exit", 183),
    ("calls/runtime_called_machine_loop_search_exit", 70),
    ("calls/runtime_dispatch_binary_call_argument_exit", 70),
    ("calls/runtime_exit_code_exit", 70),
    ("calls/runtime_explicit_discard_executes_exit", 70),
    ("calls/runtime_free_machine_looping_value_call_exit", 70),
    ("calls/runtime_free_machine_value_call_exit", 70),
    ("calls/runtime_free_machine_value_call_mut_arg_exit", 70),
    ("calls/runtime_local_string_field_copy_through_mut_exit", 70),
    ("calls/runtime_multi_arm_value_transition_exit", 70),
    ("calls/runtime_mutable_dynamic_indexed_machine_owned_parameter_write_exit", 175),
    ("calls/runtime_mutable_local_indexed_parameter_write_exit", 171),
    ("calls/runtime_mutable_local_parameter_write_exit", 171),
    ("calls/runtime_mutable_machine_owned_local_indexed_parameter_write_exit", 173),
    ("calls/runtime_mutable_machine_owned_parameter_write_exit", 141),
    ("calls/runtime_mutable_parameter_read_modify_write_exit", 191),
    ("calls/runtime_nested_called_machine_loop_exit", 70),
    ("calls/runtime_nested_guarded_reference_returned_slice_element_exit", 184),
    ("calls/runtime_nested_value_call_in_substate_exit", 70),
    ("calls/runtime_offset_string_call_results_through_reference_fields_exit", 196),
    ("calls/runtime_recursive_value_return_exit", 70),
    ("calls/runtime_referenced_local_outlives_sibling_guard_call_exit", 70),
    ("calls/runtime_reference_param_forwarded_through_loop_exit", 70),
    ("calls/runtime_reference_returned_slice_element_through_param_exit", 70),
    ("calls/runtime_reference_returned_slice_element_write_exit", 181),
    ("calls/runtime_string_call_result_through_reference_field_exit", 186),
    ("calls/runtime_transition_subject_call_single_evaluation_exit", 70),
    ("calls/runtime_two_string_call_results_through_reference_fields_exit", 194),
    ("calls/runtime_value_call_single_execution_exit", 70),
    ("calls/runtime_value_call_slice_len_guard_exit", 70),
    ("calls/runtime_value_call_through_alias_in_dispatch_exit", 70),
    ("calls/runtime_value_position_branching_call_exit", 70),
    ("calls/runtime_value_transition_unsigned_guard_exit", 70),
    ("control_flow/fixed_array_element_guard", 0),
    ("control_flow/runtime_boolean_or_guard_exit", 71),
    ("control_flow/runtime_case_member_dispatch_exit", 70),
    ("control_flow/runtime_boolean_transition_argument_after_string_guard_exit", 247),
    ("control_flow/runtime_direct_boolean_transition_argument_exit", 211),
    ("control_flow/runtime_effectful_subject_single_evaluation_exit", 70),
    ("control_flow/runtime_local_boolean_conjunction_value_exit", 74),
    ("control_flow/runtime_local_boolean_or_value_exit", 251),
    ("control_flow/runtime_local_boolean_transition_argument_exit", 201),
    ("control_flow/runtime_local_scalar_comparison_value_exit", 76),
    ("control_flow/runtime_local_string_comparison_value_exit", 78),
    ("control_flow/runtime_negated_boolean_place_guard_exit", 73),
    ("control_flow/runtime_negated_comparison_guard_exit", 75),
    ("control_flow/runtime_state_loop_indexed_search_exit", 70),
    ("control_flow/runtime_straight_line_terminal_field_readback_exit", 70),
    ("control_flow/runtime_straight_line_terminal_local_exit", 70),
    ("control_flow/runtime_tuple_transition_exit", 22),
    ("data/case_membership_union_guard_exit", 70),
    ("data/case_membership_value_exit", 70),
    ("data/case_payload_native_construction", 70),
    ("data/runtime_case_payload_guard_read_exit", 70),
    ("data/runtime_case_reassignment_exit", 70),
    ("data/runtime_data_properties_exit", 70),
    ("domains/executable_domain_membership_expression_exit", 81),
    ("domains/executable_domain_membership_intersection_guard_exit", 231),
    ("domains/executable_domain_membership_intersection_value_exit", 233),
    ("domains/executable_domain_membership_union_guard_exit", 241),
    ("domains/executable_domain_membership_union_value_exit", 205),
    ("domains/executable_imported_domain_membership_exit", 91),
    ("domains/executable_imported_domain_membership_guard_exit", 81),
    ("domains/executable_imported_domain_membership_intersection_guard_exit", 219),
    ("domains/executable_imported_domain_membership_intersection_value_exit", 217),
    ("domains/executable_imported_domain_membership_union_guard_exit", 217),
    ("domains/executable_imported_domain_membership_union_value_exit", 215),
    ("dungeon/runtime_clear_carve_render_string_fields_exit", 198),
    ("dungeon/runtime_direct_boolean_conjunction_exit", 21),
    ("dungeon/runtime_enemy_clear_reentry_exit", 51),
    ("dungeon/runtime_full_level_wrapper_lookup_string_field_exit", 202),
    ("dungeon/runtime_guarded_inline_leaf_arm_skip_exit", 70),
    ("dungeon/runtime_multi_room_reentry_exit", 63),
    ("dungeon/runtime_ordered_room_dispatch_after_call_exit", 83),
    ("dungeon/runtime_ordered_room_dispatch_exit", 73),
    ("dungeon/runtime_ordered_room_dispatch_game_shape_exit", 93),
    ("dungeon/runtime_ordered_room_dispatch_large_machine_exit", 103),
    ("dungeon/runtime_room_use_reentry_exit", 41),
    ("expressions/runtime_call_result_binary_operand_exit", 70),
    ("expressions/runtime_cast_operand_exit", 70),
    ("expressions/runtime_f32_arithmetic_exit", 70),
    ("expressions/runtime_f32_local_arithmetic_exit", 70),
    ("expressions/runtime_field_default_exit", 70),
    ("expressions/runtime_fixed_array_field_guard_exit", 70),
    ("expressions/runtime_fixed_array_field_value_exit", 70),
    ("expressions/runtime_float_arithmetic_exit", 70),
    ("expressions/runtime_float_comparison_exit", 70),
    ("expressions/runtime_float_constant_store_exit", 70),
    ("expressions/runtime_float_local_arithmetic_exit", 70),
    ("expressions/runtime_float_place_comparison_exit", 70),
    ("expressions/runtime_literal_source_cast_exit", 70),
    ("expressions/runtime_match_value_exit", 70),
    ("expressions/runtime_numeric_cast_exit", 70),
    ("operators/compound_assignment_exit", 70),
    ("operators/integer_literal_suffix_exit", 70),
    ("operators/runtime_shift_operators_exit", 70),
    ("operators/unary_negation_exit", 70),
    ("slices/runtime_dispatch_mutable_slice_element_write_exit", 31),
    ("slices/runtime_frame_array_slice_parameter_alias_exit", 72),
    ("slices/runtime_local_slice_len_comparison_value_exit", 191),
    ("slices/runtime_mutable_slice_element_write_exit", 21),
    ("slices/runtime_mutable_slice_element_write_straight_line_exit", 70),
    ("slices/runtime_nested_subslice_dynamic_index_exit", 213),
    ("slices/runtime_nested_subslice_fixed_index_exit", 215),
    ("slices/runtime_slice_fixed_index_guard_exit", 121),
    ("slices/runtime_slice_index_copy_dispatch_exit", 61),
    ("slices/runtime_slice_index_copy_exit", 51),
    ("slices/runtime_slice_index_read_dispatch_exit", 43),
    ("slices/runtime_slice_index_read_exit", 41),
    ("slices/runtime_slice_index_transition_exit", 111),
    ("slices/runtime_slice_iteration_exit", 91),
    ("slices/runtime_slice_len_transition_exit", 101),
    ("slices/runtime_subslice_bounded_dynamic_index_exit", 209),
    ("slices/runtime_subslice_bounded_range_len_exit", 215),
    ("slices/runtime_subslice_dynamic_index_exit", 207),
    ("slices/runtime_subslice_end_dynamic_index_exit", 211),
    ("slices/runtime_subslice_of_slice_param_exit", 70),
    ("slices/runtime_subslice_range_len_exit", 203),
    ("slices/runtime_subslice_range_pointer_exit", 205),
    ("storage/runtime_dispatch_helper_local_alias_add_exit", 181),
    ("storage/runtime_dispatch_local_index_binary_write_exit", 191),
    ("storage/runtime_machine_owned_fixed_indexed_struct_copy_exit", 83),
    ("storage/runtime_machine_owned_indexed_integer_write_exit", 79),
    ("storage/runtime_machine_owned_indexed_nested_exit_write_exit", 89),
    ("storage/runtime_machine_owned_indexed_nested_room_copy_exit", 87),
    ("storage/runtime_machine_owned_indexed_struct_copy_exit", 85),
    ("storage/runtime_slice_alias_indexed_field_write_exit", 201),
    ("termination/runtime_shrinking_slice_recursion_exit", 70),
    ("text/runtime_call_argument_struct_string_field_slice_alias_exit", 77),
    ("text/runtime_chained_string_append_exit", 70),
    ("text/runtime_large_lookup_struct_field_concat_exit", 192),
    ("text/runtime_large_room_lookup_struct_field_concat_exit", 200),
    ("text/runtime_local_struct_string_field_concat_exit", 188),
    ("text/runtime_lookup_struct_field_concat_exit", 190),
    ("text/runtime_machine_owned_indexed_string_field_concat_exit", 81),
    ("text/runtime_machine_string_append_in_place_exit", 70),
    ("text/runtime_mutable_string_parameter_concat_exit", 77),
    ("text/runtime_mutable_string_parameter_concat_write_line", 77),
    ("text/runtime_mutable_string_parameter_wrapped_concat_write_line", 77),
    ("text/runtime_mutable_struct_string_field_copy_concat_exit", 77),
    ("text/runtime_mutable_struct_string_field_copy_concat_write_line", 77),
    ("text/runtime_slice_alias_indexed_string_field_concat_exit", 77),
    ("text/runtime_stderr_write_exit", 70),
    ("text/runtime_string_concat_membership_exit", 71),
    ("text/runtime_string_concat_two_fields_exit", 70),
    ("text/runtime_string_field_concat_exit", 73),
    ("traits/equatable_record_equality_exit", 70),
    ("traits/equatable_sum_payload_equality_exit", 70),
    ("traits/runtime_conformance_item_exit", 70),
    ("traits/runtime_dyn_single_impl_dispatch_exit", 70),
    ("traits/runtime_dyn_two_impl_dispatch_exit", 70),
    ("traits/runtime_dyn_two_impl_dispatch_swapped_exit", 70),
    ("traits/runtime_ref_param_method_dispatch_exit", 70),
    ("traits/runtime_typed_two_method_receivers_exit", 70),
    ("types/runtime_i8_signed_arith_exit", 70),
    ("types/runtime_i16_signed_arith_exit", 70),
    ("types/runtime_isize_signed_arith_exit", 70),
    ("types/runtime_u8_field_arith_exit", 70),
    ("types/runtime_u16_field_arith_exit", 70),
    ("versioning/runtime_version_migration_exit", 70),
    ("wire/runtime_wire_encode_era_discriminator_exit", 70),
    ("wire/runtime_wire_encode_primitive_exit", 70),
];

/// Run canaries the suite executes that are DELIBERATELY not in `RUN_CANARIES`,
/// each with the reason. The drift guard below asserts that every run canary the
/// suite asserts an exit code for appears in exactly one of the two lists, so an
/// exclusion can never be silent (and a stale exclusion fails the guard too).
///
/// `(relative path under canaries/pass, reason for exclusion)`.
const EXCLUDED_RUN_CANARIES: &[(&str, &str)] = &[
    (
        "dungeon/runtime_ordered_room_dispatch_loop_exit",
        "suite feeds stdin (b\"east\\n\"); differential harness runs with empty stdin, so the recorded exit code 135 does not apply",
    ),
    (
        "dungeon/runtime_ordered_room_dispatch_real_show_states_exit",
        "suite feeds stdin (b\"east\\n\"); differential harness runs with empty stdin, so the recorded exit code 145 does not apply",
    ),
    (
        "text/runtime_stdin_command_branch_exit",
        "suite feeds stdin (b\"look\\n\" command); differential harness runs with empty stdin, so the recorded exit code 0 does not apply",
    ),
    (
        "text/runtime_stdin_line_buffering_exit",
        "suite feeds stdin (b\"hello\\nworld\\n\", plus a CRLF variant test reusing this canary); differential harness runs with empty stdin, so the recorded exit code 0 does not apply",
    ),
    (
        "dungeon/runtime_threaded_mut_arg_interrupt_soak_exit",
        "interrupt-timing soak: fifty million dispatched iterations target NATIVE kernel-preemption windows (the Darwin x18 scratch-register regression); interpreting that loop adds minutes of pure overhead with no oracle value",
    ),
];

/// Drift guard: re-parse `canary_suite.rs` at test time and assert every
/// `*_canary_runs` test that calls `pass_canary(..)` and asserts `Some(code)` is
/// mirrored in `RUN_CANARIES` (or explicitly listed in `EXCLUDED_RUN_CANARIES`).
/// Fails with copy-paste-ready entries when the lists drift.
#[test]
fn run_canary_list_matches_canary_suite() {
    use std::collections::{BTreeMap, BTreeSet};

    let suite_path = repo_root()
        .join("compiler/orchestration/omega-compiler/tests/canary_suite.rs");
    let source = fs::read_to_string(&suite_path).unwrap_or_else(|error| {
        panic!("failed to read canary suite source at {}: {error}", suite_path.display())
    });

    let parsed = parse_suite_run_canaries(&source);
    assert!(
        parsed.len() >= 130,
        "parsed only {} (path, code) pairs from {} -- the canary_suite.rs parser \
         in this test has likely regressed (expected at least 130)",
        parsed.len(),
        suite_path.display()
    );

    let expected: BTreeMap<&str, i32> = RUN_CANARIES.iter().copied().collect();
    let excluded: BTreeMap<&str, &str> = EXCLUDED_RUN_CANARIES.iter().copied().collect();
    let parsed_paths: BTreeSet<&str> = parsed.iter().map(|(path, _)| path.as_str()).collect();

    let mut problems: Vec<String> = Vec::new();

    // Suite canaries that are neither mirrored nor explicitly excluded.
    let missing: Vec<&(String, i32)> = parsed
        .iter()
        .filter(|(path, _)| {
            !expected.contains_key(path.as_str()) && !excluded.contains_key(path.as_str())
        })
        .collect();
    if !missing.is_empty() {
        let lines: Vec<String> = missing
            .iter()
            .map(|(path, code)| format!("    (\"{path}\", {code}),"))
            .collect();
        problems.push(format!(
            "MISSING from RUN_CANARIES (paste into the sorted list, or add to \
             EXCLUDED_RUN_CANARIES with a reason):\n{}",
            lines.join("\n")
        ));
    }

    // Mirrored canaries whose suite-asserted exit code drifted.
    let mut wrong_code: Vec<String> = Vec::new();
    for (path, code) in &parsed {
        if let Some(recorded) = expected.get(path.as_str()) {
            if recorded != code {
                wrong_code.push(format!(
                    "    (\"{path}\", {code}),  // RUN_CANARIES records {recorded}"
                ));
            }
        }
    }
    if !wrong_code.is_empty() {
        problems.push(format!(
            "EXIT CODE DRIFT (suite now asserts a different code; update RUN_CANARIES):\n{}",
            wrong_code.join("\n")
        ));
    }

    // RUN_CANARIES entries the suite no longer runs.
    let stale: Vec<String> = RUN_CANARIES
        .iter()
        .filter(|(path, _)| !parsed_paths.contains(path))
        .map(|(path, code)| format!("    (\"{path}\", {code}),"))
        .collect();
    if !stale.is_empty() {
        problems.push(format!(
            "STALE in RUN_CANARIES (no matching `pass_canary` run canary in the suite; remove):\n{}",
            stale.join("\n")
        ));
    }

    // Exclusions must reference a live suite canary and must not shadow RUN_CANARIES.
    for (path, reason) in EXCLUDED_RUN_CANARIES {
        if !parsed_paths.contains(path) {
            problems.push(format!(
                "STALE EXCLUSION: (\"{path}\", \"{reason}\") -- the suite no longer runs it; remove from EXCLUDED_RUN_CANARIES"
            ));
        }
        if expected.contains_key(path) {
            problems.push(format!(
                "OVERLAP: \"{path}\" is in both RUN_CANARIES and EXCLUDED_RUN_CANARIES; pick one"
            ));
        }
    }

    assert!(
        problems.is_empty(),
        "RUN_CANARIES drifted from canary_suite.rs ({} suite run canaries parsed):\n\n{}",
        parsed.len(),
        problems.join("\n\n")
    );
}

/// Extract every `(pass_canary path, asserted exit code)` pair from the canary
/// suite source: function boundaries via `fn <name>_canary_runs(`, then the first
/// `pass_canary("..")` and the first `Some(<digits>)` within that function's text
/// (up to the next top-level `fn `). Functions without a `pass_canary` call (e.g.
/// sample-based run tests) or without a numeric `Some(..)` assertion are skipped.
/// Pairs are deduplicated (one canary program can back multiple suite tests).
fn parse_suite_run_canaries(source: &str) -> Vec<(String, i32)> {
    use std::collections::BTreeSet;

    // Byte offsets of every top-level-looking `fn ` (start of file or preceded by a
    // newline); each function's text runs until the next such offset.
    let mut fn_starts: Vec<usize> = source
        .match_indices("\nfn ")
        .map(|(index, _)| index + 1)
        .collect();
    if source.starts_with("fn ") {
        fn_starts.insert(0, 0);
    }

    let mut pairs: BTreeSet<(String, i32)> = BTreeSet::new();
    for (position, &start) in fn_starts.iter().enumerate() {
        let end = fn_starts.get(position + 1).copied().unwrap_or(source.len());
        let body = &source[start..end];

        // `fn <name>(` -- only `_canary_runs` tests are run canaries.
        let after_fn = &body["fn ".len()..];
        let name_end = after_fn
            .find(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
            .unwrap_or(after_fn.len());
        let name = &after_fn[..name_end];
        if !name.ends_with("_canary_runs") || !after_fn[name_end..].starts_with('(') {
            continue;
        }

        // First pass_canary("...") -- absent for sample-based tests: skip those.
        let Some(path) = body
            .split_once("pass_canary(\"")
            .and_then(|(_, rest)| rest.split_once('"'))
            .map(|(path, _)| path)
        else {
            continue;
        };

        // First `Some(` whose payload is a numeric literal (the asserted exit code).
        let Some(code) = first_numeric_some(body) else {
            continue;
        };

        pairs.insert((path.to_string(), code));
    }
    pairs.into_iter().collect()
}

/// First `Some(<digits>)` in `text`, parsed as the exit code.
fn first_numeric_some(text: &str) -> Option<i32> {
    let mut remaining = text;
    while let Some((_, rest)) = remaining.split_once("Some(") {
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if !digits.is_empty() && rest[digits.len()..].starts_with(')') {
            return digits.parse().ok();
        }
        remaining = rest;
    }
    None
}

#[test]
fn interpreter_matches_native_on_supported_canaries() {
    let mut matched: Vec<String> = Vec::new();
    let mut skipped: Vec<(String, String)> = Vec::new();
    let mut mismatches: Vec<String> = Vec::new();

    for (name, expected_code) in RUN_CANARIES {
        if std::env::var("DIFF_TRACE").is_ok() {
            eprintln!("[trace] {name}");
        }
        let main_path = pass_canary(name).join("main.omg");

        let checked = match compile_to_checked(&main_path, None) {
            Ok(checked) => checked,
            Err(diagnostics) => panic!(
                "{name}: frontend compile to checked failed:\n{}",
                join_diagnostics(&diagnostics)
            ),
        };

        let outcome = interpret(&checked, b"");
        if outcome.is_error() {
            skipped.push((name.to_string(), outcome.error.clone().unwrap_or_default()));
            continue;
        }

        let (native_code, native_stdout, native_stderr) =
            compile_and_run_native(name, &main_path);

        // Native is the source of truth, but sanity-check the suite's documented code too:
        // if native disagrees with the recorded expected code the corpus drifted.
        assert_eq!(
            native_code, *expected_code,
            "{name}: native exit {native_code} != suite-recorded expected {expected_code} \
             (RUN_CANARIES is stale)"
        );

        let mut local_failures = Vec::new();
        if outcome.exit_code != native_code {
            local_failures.push(format!(
                "exit code: interp {} != native {native_code}",
                outcome.exit_code
            ));
        }
        if outcome.stdout != native_stdout {
            local_failures.push(format!(
                "stdout: interp {:?} != native {:?}",
                String::from_utf8_lossy(&outcome.stdout),
                String::from_utf8_lossy(&native_stdout)
            ));
        }
        if outcome.stderr != native_stderr {
            local_failures.push(format!(
                "stderr: interp {:?} != native {:?}",
                String::from_utf8_lossy(&outcome.stderr),
                String::from_utf8_lossy(&native_stderr)
            ));
        }

        if local_failures.is_empty() {
            matched.push(name.to_string());
        } else {
            mismatches.push(format!("{name}:\n    {}", local_failures.join("\n    ")));
        }
    }

    eprintln!(
        "\ndifferential oracle over {} RUN canaries:\n  {} matched (interp==native)\n  {} skipped (interpreter unsupported)\n  {} MISMATCH",
        RUN_CANARIES.len(),
        matched.len(),
        skipped.len(),
        mismatches.len(),
    );
    eprintln!("\ntop unsupported constructs (interpreter skips):");
    for (reason, count) in summarize_reasons(&skipped, 15) {
        eprintln!("  {count:>3}  {reason}");
    }
    eprintln!("\nmatched canaries ({}):", matched.len());
    for name in &matched {
        eprintln!("  {name}");
    }

    assert!(
        !matched.is_empty(),
        "expected at least one canary where interpreter == native"
    );

    assert!(
        mismatches.is_empty(),
        "interpreter disagreed with native on {} passing RUN canaries (these are INTERPRETER bugs -- native is correct on a passing canary):\n\n{}",
        mismatches.len(),
        mismatches.join("\n\n")
    );
}

/// The `cli_mvp` sample is a stable end-to-end program (prints "Hello, Omega." and exits
/// 0). It exercises the imported-std `console` host-boundary path (write_line + exit_process)
/// that the canaries' inline boundary traits do not, so it is a useful permanent guard that
/// the interpreter and native agree there too.
#[test]
fn interpreter_matches_native_on_cli_mvp_sample() {
    let main_path = repo_root().join("samples").join("cli_mvp").join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|diagnostics| panic!("cli_mvp compile failed:\n{}", join_diagnostics(&diagnostics)));

    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "cli_mvp should be fully supported by the interpreter, got: {:?}",
        outcome.error
    );

    let (native_code, native_stdout, native_stderr) =
        compile_and_run_native("cli_mvp", &main_path);
    assert_eq!(outcome.exit_code, native_code, "cli_mvp exit code");
    assert_eq!(
        outcome.stdout, native_stdout,
        "cli_mvp stdout: interp {:?} != native {:?}",
        String::from_utf8_lossy(&outcome.stdout),
        String::from_utf8_lossy(&native_stdout)
    );
    assert_eq!(
        outcome.stderr, native_stderr,
        "cli_mvp stderr: interp {:?} != native {:?}",
        String::from_utf8_lossy(&outcome.stderr),
        String::from_utf8_lossy(&native_stderr)
    );
}

/// The dungeon script the canary suite's PE test could use to visit R00..R04: four `north`
/// moves walk the main hall chain, then `quit`.
const DUNGEON_SCRIPT: &[u8] = b"north\r\nnorth\r\nnorth\r\nnorth\r\nquit\r\n";

/// The full dungeon sample, interpreted end-to-end, pins the ORACLE-SIDE GROUND TRUTH for
/// the known native deep-room bug. Room descriptions are depth-derived
/// (`MazeBuilder::room_description`: depth <= 2 shallow limestone, depth <= 5 winding
/// branch, deeper tiers darker), and the interpreter renders R03/R04 (depth 3/4) with the
/// "winding branch" text. The NATIVE binary currently renders the depth<=2 text there
/// (lost `&mut Level` mutations through dispatched generation), which is why this asserts
/// against the interpreter only -- see `interpreter_matches_native_on_dungeon_sample`.
#[test]
fn interpreter_dungeon_renders_depth_correct_rooms() {
    let main_path = repo_root()
        .join("samples")
        .join("dungeon_crawler_cli")
        .join("main.omg");
    let checked = compile_to_checked(&main_path, None).unwrap_or_else(|diagnostics| {
        panic!(
            "dungeon compile to checked failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });

    let outcome = interpret(&checked, DUNGEON_SCRIPT);
    assert!(
        !outcome.is_error(),
        "the dungeon should be fully supported by the interpreter, got: {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 0, "dungeon should exit 0 after `quit`");
    let stdout = String::from_utf8_lossy(&outcome.stdout).into_owned();

    // R00, the gate.
    assert!(stdout.contains("== Gate =="), "gate title\n{stdout}");
    assert!(
        stdout.contains("A bottomless dark room near the dungeon heart."),
        "gate description\n{stdout}"
    );
    // R01/R02 are depth <= 2.
    assert!(
        stdout.contains("A shallow limestone room with fresh claw marks."),
        "shallow-tier description\n{stdout}"
    );
    // R03/R04 are depth 3/4 -- the tier the native backend currently collapses to the
    // shallow text. This line is the ground truth the backend fix must reproduce.
    assert!(
        stdout.contains("A winding branch room where the walls sweat mineral dust."),
        "deep rooms (R03/R04) must render the depth-3..5 description\n{stdout}"
    );
    // The per-line formatter machines (title/event/paths) write through `&mut String`
    // params forwarded INTO transition-target states; guard that they render their own
    // text rather than echoing the stale description (multi-hop ref-forwarding).
    assert!(stdout.contains("The room is quiet."), "event line\n{stdout}");
    assert!(
        stdout.contains("[Paths] south | north | east"),
        "paths line\n{stdout}"
    );
    assert!(
        stdout.contains("A dungeon bat blocks the way. Type fight."),
        "enemy event line (R03)\n{stdout}"
    );
    assert!(
        stdout.contains("A fountain of life glows here. Type use to drink."),
        "fountain event line (R04)\n{stdout}"
    );
}

/// Strict interpreter-vs-native equality over the dungeon sample: the full game binary
/// and the reference interpreter must produce byte-identical stdout (and the same exit
/// code) for a deep `north x4` walk. This is the end-to-end differential guard that
/// caught the frame-stacker sibling-overlay bug (generation silently stalled after the
/// first `should_carve` guard, so R03+ rendered the shallow description).
#[test]
fn interpreter_matches_native_on_dungeon_sample() {
    let main_path = repo_root()
        .join("samples")
        .join("dungeon_crawler_cli")
        .join("main.omg");
    let checked = compile_to_checked(&main_path, None).unwrap_or_else(|diagnostics| {
        panic!(
            "dungeon compile to checked failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, DUNGEON_SCRIPT);
    assert!(
        !outcome.is_error(),
        "the dungeon should be fully supported by the interpreter, got: {:?}",
        outcome.error
    );

    let (native_code, native_stdout, native_stderr) =
        compile_and_run_native_with_stdin("dungeon_crawler_cli", &main_path, DUNGEON_SCRIPT);
    assert_eq!(outcome.exit_code, native_code, "dungeon exit code");
    assert_eq!(
        String::from_utf8_lossy(&outcome.stdout),
        String::from_utf8_lossy(&native_stdout),
        "dungeon stdout: interpreter (left) vs native (right)"
    );
    assert_eq!(
        String::from_utf8_lossy(&outcome.stderr),
        String::from_utf8_lossy(&native_stderr),
        "dungeon stderr: interpreter (left) vs native (right)"
    );
}

/// Collapse skip reasons to a normalized phrase so we can rank the most common unsupported
/// constructs (program-specific identifiers in backticks are stripped).
fn summarize_reasons(skipped: &[(String, String)], top: usize) -> Vec<(String, usize)> {
    use std::collections::BTreeMap;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for (_, reason) in skipped {
        *counts.entry(normalize_reason(reason)).or_default() += 1;
    }
    let mut ranked: Vec<(String, usize)> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked.truncate(top);
    ranked
}

fn normalize_reason(reason: &str) -> String {
    let mut out = String::new();
    let mut in_quote = false;
    for ch in reason.chars() {
        if ch == '`' {
            in_quote = !in_quote;
            if in_quote {
                out.push_str("`…`");
            }
            continue;
        }
        if !in_quote {
            out.push(ch);
        }
    }
    // Drop any debug-formatted tail (e.g. `Indexed(TableIndexedExpression { ... })`).
    if let Some(index) = out.find('(') {
        out.truncate(index);
    }
    out.trim().to_owned()
}

fn compile_and_run_native(canary_name: &str, main_path: &Path) -> (i32, Vec<u8>, Vec<u8>) {
    compile_and_run_native_with_stdin(canary_name, main_path, b"")
}

/// Compile + run the native binary with the given stdin; returns
/// `(exit code, stdout bytes, stderr bytes)`.
fn compile_and_run_native_with_stdin(
    canary_name: &str,
    main_path: &Path,
    stdin: &[u8],
) -> (i32, Vec<u8>, Vec<u8>) {
    let build_dir = std::env::temp_dir().join(format!(
        "omega-interp-diff-{}-{}",
        canary_name.replace(['/', '\\'], "_"),
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path.to_path_buf(),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|diagnostics| {
        panic!(
            "{canary_name}: native compile failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });

    let mut child = Command::new(build_dir.join(executable_name()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("{canary_name}: native spawn failed: {error}"));
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(stdin)
        .unwrap_or_else(|error| panic!("{canary_name}: native stdin write failed: {error}"));
    let output = child
        .wait_with_output()
        .unwrap_or_else(|error| panic!("{canary_name}: native run failed: {error}"));

    let code = output.status.code().unwrap_or(-1);
    let stdout = output.stdout.clone();
    let stderr = output.stderr.clone();
    let _ = fs::remove_dir_all(&build_dir);
    (code, stdout, stderr)
}

fn join_diagnostics(diagnostics: &[omega_core::diagnostics::Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("interpreter crate should live under compiler/orchestration/omega-interpreter")
        .to_path_buf()
}

fn pass_canary(path: &str) -> PathBuf {
    repo_root().join("canaries/pass").join(path)
}

#[cfg(windows)]
fn executable_name() -> &'static str {
    "omega-program.exe"
}

#[cfg(not(windows))]
fn executable_name() -> &'static str {
    "omega-program"
}
