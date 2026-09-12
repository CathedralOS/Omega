//! Bounded input is shared checked library code, not a target line intrinsic.

use super::*;
use compiler::CheckedCompileRequest;

#[test]
fn selected_console_line_reader_callers_normalize_only_the_reported_prefix() {
    let echo = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &pass_canary("text/runtime_stdin_line_buffering_exit").join("main.omg"),
        Some("macos_arm64"),
    ))
    .expect("two-read echo reaches checked semantics");
    for input in [
        b"hello\nworld\n".as_slice(),
        b"hello\r\nworld\r\n",
        b"hello\nworld",
    ] {
        let result = interpret(&echo, input);
        assert_eq!(result.error, None);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, b"hello\nworld\n");
    }
    let raw = interpret(&echo, b"\xff\0\rX\n\xc3\xa9\n");
    assert_eq!(raw.error, None);
    assert_eq!(raw.exit_code, 0);
    assert_eq!(raw.stdout, b"\xff\0\rX\n\xc3\xa9\n");
    let full = interpret(&echo, &[b'x'; 64]);
    assert_eq!(full.error, None);
    assert_eq!(
        full.exit_code, 1,
        "Full cannot be normalized as a complete line"
    );
    assert!(full.stdout.is_empty());

    let command = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &pass_canary("text/runtime_stdin_command_branch_exit").join("main.omg"),
        Some("macos_arm64"),
    ))
    .expect("ASCII command input reaches checked semantics");
    for input in [b"look\n".as_slice(), b"look\r\n", b"look"] {
        let result = interpret(&command, input);
        assert_eq!(result.error, None);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, b"look\n");
    }
    for input in [b"look\r".as_slice(), b"look\0\n", &[b'x'; 32]] {
        let result = interpret(&command, input);
        assert_eq!(result.error, None);
        assert_eq!(
            result.exit_code, 1,
            "input {input:?}, output {:?}",
            result.stdout
        );
        assert_eq!(result.stdout, b"invalid\n");
    }
}

#[test]
fn selected_console_line_reader_sample_callers_reach_checked_semantics() {
    // These include every pause-buffer representation changed by the API
    // migration, not unrelated sample features. The interactive dungeon retains
    // independently witnessed frontend failures and is checked separately.
    for sample in [
        "cli/basics/cli_mvp",
        "cli/basics/text_greeting",
        "cli/systems/atomics_cross",
        "cli/systems/status_report",
        "cli/text/text_padding",
    ] {
        let root = sample_project(sample).join("main.omg");
        compile_reviewed_repository_fixture(CheckedCompileRequest::new(&root, Some("macos_arm64")))
            .unwrap_or_else(|diagnostics| panic!("{sample}: {diagnostics:#?}"));
    }
}

#[test]
fn selected_console_line_reader_migrated_fixtures_reach_checked_semantics() {
    let mut failures = Vec::new();
    for fixture in [
        "pass/calls/mutable_output_host_call",
        "pass/calls/runtime_indexed_copy_aggregate_handoff_exit",
        "pass/calls/runtime_mutable_call_before_transition_args_exit",
        "pass/dungeon/runtime_ordered_room_dispatch_large_machine_exit",
        "pass/dungeon/runtime_ordered_room_dispatch_loop_exit",
        "pass/dungeon/runtime_ordered_room_dispatch_real_show_states_exit",
        "pass/host/runtime_console_line_descriptor_exit",
        "pass/host/runtime_console_line_fixed_array_exit",
        "pass/text/runtime_stdin_command_branch_exit",
        "pass/text/runtime_stdin_line_buffering_exit",
        "pass/text/runtime_text_storage",
        "run/nested_struct_command_branch",
        "run/nested_struct_prompt_command_branch",
    ] {
        let root = repo_root()
            .join("tests/omega")
            .join(fixture)
            .join("main.omg");
        if let Err(diagnostics) = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
            &root,
            Some("macos_arm64"),
        )) {
            failures.push(format!("{fixture}: {diagnostics:#?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn selected_console_line_reader_is_a_shared_checked_adapter() {
    let canary = pass_canary(fixture_roster::RUNTIME_ADAPTER_FORWARDING_EXIT);
    for target in [
        "windows_x86_64",
        "macos_arm64",
        "linux_x86_64",
        "linux_arm64",
    ] {
        let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
            &canary.join("main.omg"),
            Some(target),
        ))
        .unwrap_or_else(|diagnostics| panic!("line reader provider: {diagnostics:#?}"));
        let plan = checked
            .selected_provider_plans()
            .plans()
            .iter()
            .find(|plan| plan.schema.trait_name == "Console")
            .unwrap();
        let row = plan
            .rows
            .iter()
            .find(|row| row.method == "read_line")
            .unwrap();
        let expected = checked_adapter_identity(&checked, "ConsoleNativeProvider::read_line");
        assert!(
            matches!(&row.binding, effects::provider_plan::ProviderBinding::CheckedAdapter { machine_identity, .. } if machine_identity == &expected),
            "bounded line assembly must be an ordinary checked provider body"
        );
        assert_eq!(plan.provider_type, "ConsoleNativeProvider");
        assert!(plan.covers_schema());
        let adapter = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "ConsoleNativeProvider::read_line")
            .unwrap();
        let entry = &checked.machine_states(adapter)[0];
        assert_eq!(
            checked
                .state_parameters(entry)
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<Vec<_>>(),
            ["out_line"]
        );
    }
}

#[test]
fn selected_console_line_reader_preserves_raw_prefix_count_and_unread_suffix() {
    let canary = pass_canary("host/runtime_console_bounded_line_exit");
    for target in [
        "windows_x86_64",
        "macos_arm64",
        "linux_x86_64",
        "linux_arm64",
    ] {
        let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
            &canary.join("main.omg"),
            Some(target),
        ))
        .unwrap_or_else(|diagnostics| panic!("{target}: {diagnostics:#?}"));
        let cases: &[(&[u8], i32, &[u8])] = &[
            (b"", 20, b"\xa5\xa5\xa5\xa5"),
            (b"\nX", 11, b"\n\xa5\xa5\xa5X"),
            (b"ab", 22, b"ab\xa5\xa5"),
            (b"abc\nX", 14, b"abc\nX"),
            (b"abcd\n", 34, b"abcd\n"),
            (b"abcd", 34, b"abcd"),
            (b"\0\r\n\xff", 13, b"\0\r\n\xa5\xff"),
            (b"\xc3\xa9\xff\0X", 34, b"\xc3\xa9\xff\0X"),
        ];
        for &(input, expected_exit, expected_output) in cases {
            let outcome = interpret(&checked, input);
            assert_eq!(outcome.error, None, "{target}: input {input:?}");
            assert_eq!(
                outcome.exit_code, expected_exit,
                "{target}: input {input:?}"
            );
            assert_eq!(outcome.stdout, expected_output, "{target}: input {input:?}");
        }
        for byte in 0..=255u8 {
            let outcome = interpret(&checked, &[byte, b'\n', 0xff]);
            let (expected_exit, expected_output) = if byte == b'\n' {
                (11, vec![byte, 0xa5, 0xa5, 0xa5, b'\n'])
            } else {
                (12, vec![byte, b'\n', 0xa5, 0xa5, 0xff])
            };
            assert_eq!(outcome.error, None, "{target}: byte {byte}");
            assert_eq!(outcome.exit_code, expected_exit, "{target}: byte {byte}");
            assert_eq!(outcome.stdout, expected_output, "{target}: byte {byte}");
        }
        let repeated = interpret_entry(&checked, "Main::repeat", b"ab\nc");
        assert_eq!(repeated.error, None, "{target}: repeated middle window");
        assert_eq!(repeated.exit_code, 70);
        assert_eq!(repeated.stdout, b"\xa5ab\xa5\xa5\nb\xa5");
    }
}
