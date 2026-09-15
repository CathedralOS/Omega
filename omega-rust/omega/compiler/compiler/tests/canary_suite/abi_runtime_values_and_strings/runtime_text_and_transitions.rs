use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, Command, Stdio, compile,
    compile_reviewed_repository_fixture, compile_rooted_canary_for_native_host, fs, interpret,
    pass_canary, run_canary,
};
use compiler::CheckedCompileRequest;
use std::io::Write;

#[test]
fn runtime_value_call_transition_args_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_TRANSITION_ARGS_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("transition-args canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (all six params delivered), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-transition-args-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("transition-args canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("transition-args canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("transition-args canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected every value-call transition argument to deliver ITS call's \
         result (exit 70), got {:?} (71/72 = call+literal; 73/74 = same-callee \
         pair; 75/76 = different-callee pair)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The straight-line (guard-free) sibling: done(self.dbl(5), self.dbl(6))
// exits with b, which must hold ITS call's result (12) -- the historical
// bugs delivered call 1's result (10) or ZII 0.

#[test]
fn runtime_value_call_transition_args_straight_line_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_TRANSITION_ARGS_STRAIGHT_LINE_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("transition-args straight-line canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 12,
        "interpreter oracle should exit 12 (b = dbl(6)), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-transition-args-sl-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("transition-args straight-line canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("transition-args straight-line canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("transition-args straight-line canary should run");

    assert_eq!(
        output.status.code(),
        Some(12),
        "expected b = dbl(6) = 12, got {:?} (10 = b read call 1's result; \
         0 = the capture never ran)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The retired fence's own repro body, now a PASS: a guard-free
// (straight-line-scheduled) state with two same-callee value calls stored
// straight to fields. f=10 + g=12 -> sum exit 22; the historical shared-slot
// bug gave both fields the LAST result (24).

#[test]
fn runtime_value_call_shared_slot_straight_line_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_SHARED_SLOT_STRAIGHT_LINE_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("shared-slot straight-line canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 22,
        "interpreter oracle should exit 22 (f=10 + g=12), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-shared-slot-straight-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("shared-slot straight-line canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("shared-slot straight-line canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("shared-slot straight-line canary should run");

    assert_eq!(
        output.status.code(),
        Some(22),
        "expected f=10 + g=12 (exit 22), got {:?} (24 = both fields read the \
         LAST call's result -- the shared-slot bug is back)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// An enum-attached machine matching BARE `self` (`transition self {
// Signal::Green -> .. }` inside Signal::go_value, called as
// self.s.go_value()): the guard subject resolves to the attached value's
// TAG at the receiver's storage base, threaded to the CALLEE's machine via
// the expansion's branch_key (the caller's machine had resolved `self` to
// the caller's own attached data). Three discriminating cases incl. the
// non-ZII last tag + a bool designed-false leg.

#[test]
fn runtime_enum_self_method_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_ENUM_SELF_METHOD_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("enum-self-method canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (all enum-self legs discriminate), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-enum-self-method-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("enum-self-method canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("enum-self-method canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("enum-self-method canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected every bare-self enum-method leg to discriminate (exit 70), got {:?} \
         (71/72/73 = Green/Amber/Red legs; 74 = is_green designed-false took the \
         true arm)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Dispatch-bodied value calls deliver into every result position: a STRUCT
// result assigned straight to a FIELD (stored 0 until the Mutation fire
// site gave field assignments a flush point), and a FREE machine with a
// runtime-selected branch bound to a let (returned garbage before). Arm
// bodies are PURE -- the effectful-arm shape is fenced separately
// (calls/value_call_effectful_arm_rejected).

#[test]
fn runtime_value_call_dispatch_results_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_DISPATCH_RESULTS_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("dispatch-results canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (struct-to-field + free-pick legs), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-dispatch-results-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dispatch-results canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dispatch-results canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dispatch-results canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected dispatch-bodied value-call results to deliver (exit 70), got {:?} \
         (71/72 = struct-to-field components; 73/74 = free-pick false/true branch)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// An inline arm guard over a callee SLICE PARAM (`path.len > 3`) lowers via
// leaf-binding resolution: the caller's LITERAL argument substitutes into the
// guard, `.len` folds to the byte length, and the ordered comparison decides
// each arm statically -- one arm per call site, and two sites hit OPPOSITE
// arms of the same callee. The re-entrant sibling (arm targeting a
// `terminates` walk) was fenced until call-with-return landed (now the
// promoted calls/runtime_inline_recursive_walk_exit family):
// these folds and that fence landed together and must stay together.

#[test]
fn runtime_value_call_literal_len_arm_guard_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_LITERAL_LEN_ARM_GUARD_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("literal-len arm-guard canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (opposite arms across two call sites), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-literal-len-arm-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("literal-len arm-guard canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("literal-len arm-guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("literal-len arm-guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected literal-len arm guards to select one arm per site (exit 70), got {:?} \
         (71 = long-path site missed the big arm; 72 = short-path site missed small)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A scalar value-call compared to an integer literal DIRECTLY in a guard
// subject discriminates (the historical always-true face): the syntax
// lowering hoists the call into a shared let temp typed from the callee's
// declared return, and the guard compares the local. Both designed-false
// legs and the designed-true leg are checked; the effectful-subject
// single-evaluation tripwire pins that match-over-call arms share ONE temp
// (per-arm temps re-ran the callee once per attempted arm).

#[test]
fn runtime_value_call_guard_subject_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_GUARD_SUBJECT_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("guard-subject canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (all three guard legs discriminate), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-guard-subject-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("guard-subject canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("guard-subject canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("guard-subject canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected every value-call guard leg to DISCRIMINATE (exit 70), got {:?} \
         (71 = designed-false Equal took the true arm -- the always-true bug is \
         back; 72 = designed-true failed; 73 = NotEqual designed-false)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_effectful_guard_local_and_self_terminal_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_EFFECTFUL_GUARD_LOCAL_AND_SELF_TERMINAL_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("effectful guard/local and self-terminal canary should reach checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter should return both call values and execute each call once, got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-effectful-guard-local-self-terminal-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("effectful guard/local and self-terminal canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "effectful guard/local and self-terminal canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("effectful guard/local and self-terminal canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "native execution should return both call values and execute each call once, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_guarded_effectful_transition_argument_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_GUARDED_EFFECTFUL_TRANSITION_ARGUMENT_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("guarded effectful transition-argument canary should reach checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter should preserve hot/cold execution plus ordered parameter/local delivery, got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-guarded-effectful-transition-argument-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("guarded effectful transition-argument canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "guarded effectful transition-argument canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("guarded effectful transition-argument canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "native execution should preserve hot/cold execution plus ordered parameter/local delivery, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Deferral face #5: a NESTED value call inside the callee's entry
// (`self.flag = self.helper.check(1)` in Probe::make, guarded on flag,
// through an outer value call). The nested callee splices a THIRD
// source_key between the middle callee's ops, so defer/fire scans that
// stopped at the first foreign key never saw the flag mutation -- the
// outer leaf fired at the StateCall and computed the result from ZII
// flag=false. The splice run ends at the CALLER's next own op.

#[test]
fn runtime_value_call_nested_entry_call_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_NESTED_ENTRY_CALL_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("nested-entry-call canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (both nested-entry shapes deliver), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-nested-entry-call-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested-entry-call canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("nested-entry-call canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested-entry-call canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected both callee entries with nested value calls to guard on the \
         DELIVERED flag (exit 70), got {:?} (71 = nested-only entry read ZII; \
         72 = stores-around-the-nested-call variant read ZII)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// SHARED-NAME payload fields across variants through a value-call leaf
// terminal: `amount` in Deposit(amount) AND Transfer(to, amount). The
// leaf-path per-field decomposition passed case_variant: None, so `amount`
// resolved the FIRST variant's offset and Transfer's payload landed wrong
// (exit 72, silent -- the #38 collision class, previously fixed only on the
// mutation path). The write now tags payload fields with the constructed
// variant, matching the destructure side.

#[test]
fn runtime_value_call_shared_payload_name_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_SHARED_PAYLOAD_NAME_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("shared-payload-name canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (Transfer{{to:42, amount:99}} delivered), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-shared-payload-name-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("shared-payload-name canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("shared-payload-name canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("shared-payload-name canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Transfer's shared-name payload to resolve ITS variant's offsets \
         (exit 70), got {:?} (72/73 = a field landed at Deposit's offset)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A big multi-field StructLiteral payload through a value-call result slot with
// one CAST-valued field (`mode: mode as u32` -- the fs metadata_path shape,
// TASKS_FS.md blocker #2A). The leaf-path scalar write cascade had no convert
// arm, so the cast field silently dropped while its 15 siblings landed (exit 74).

#[test]
fn runtime_value_call_struct_payload_cast_field_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_STRUCT_PAYLOAD_CAST_FIELD_EXIT);
    let main_path = canary.join("main.omg");

    // Interpreter oracle first: it must agree the exit is 70.
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("value-call cast-field payload canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (full 16-field payload incl. the cast field), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-value-call-cast-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("value-call cast-field payload canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("value-call cast-field payload canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("value-call cast-field payload canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the full 16-field payload incl. the cast-valued mode field (exit 70), \
         got {:?} (74 = the cast field arrived ZII)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_branch_leaf_multiple_named_conversion_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_BRANCH_LEAF_MULTIPLE_NAMED_CONVERSION_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-branch-leaf-multiple-named-conversion-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("branch-leaf multiple named-conversion canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("branch-leaf named-conversion canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("branch-leaf multiple named-conversion canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected both conversion results to materialize before the branch-local binary initializer, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A value-call whose ENTRY host call writes self state (read_line into the
// carrier) and whose leaf arms build payloads FROM that state: the Ok arm's
// StructLiteral takes `len` from the host-written carrier; the Error arm's
// `kind` comes from a NESTED value-call guarding on it (the fs wrapper's
// `let kind = self.last_error()` shape). Regression pin for TASKS_FS.md
// blocker #2B: the arm statements (straight-line expansion) used to be emitted
// ABOVE the entry host call, so the terminal copied pre-call ZII state --
// right tag, zero payload. Both stdin legs run interpreter-first.

#[test]
fn value_call_entry_host_state_payload_canary_runs() {
    let canary = run_canary("value_call_entry_host_state_payload");
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("entry-host-state payload canary should compile to checked trees");
    for (stdin, expected) in [(&b"ok\n"[..], 70), (&b"no\n"[..], 75)] {
        let outcome = interpret(&checked, stdin);
        assert_eq!(
            outcome.exit_code,
            expected,
            "interpreter oracle should exit {expected} for stdin {:?}, got {}",
            String::from_utf8_lossy(stdin),
            outcome.exit_code
        );
    }

    let build_dir = std::env::temp_dir().join(format!(
        "omega-entry-host-state-payload-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("entry-host-state payload canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("entry-host-state payload canary should retain its executable receipt");
    for (stdin, expected) in [(&b"ok\n"[..], 70), (&b"no\n"[..], 75)] {
        let mut child = Command::new(executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("entry-host-state payload canary should start");
        child
            .stdin
            .as_mut()
            .expect("stdin should be piped")
            .write_all(stdin)
            .expect("entry-host-state payload input should be written");
        let output = child
            .wait_with_output()
            .expect("entry-host-state payload canary should finish");
        assert_eq!(
            output.status.code(),
            Some(expected),
            "expected exit {expected} for stdin {:?} (72 = Ok len read the pre-host-call ZII \
             carrier; 76 = Error kind lost through the nested value-call), got {:?}\nstderr:\n{}",
            String::from_utf8_lossy(stdin),
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 carrier command-loop with a health gate: each iteration checks health, reads
// a line into a `[u8; 16]` carrier, resolves a Command, and loops until `quit`.

#[test]
fn contained_health_loop_command_branch_carrier_canary_runs() {
    let canary = run_canary("contained_health_loop_command_branch");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-contained-health-loop-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("carrier health-loop canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("carrier health-loop canary should retain its executable receipt");
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("carrier health-loop canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"look\nzzz\nquit\n")
        .expect("carrier health-loop input should be written");
    let output = child
        .wait_with_output()
        .expect("carrier health-loop canary should finish");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected carrier health-loop canary to exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "look\ninvalid\n",
        "expected the health-gated loop to resolve each command (Look, Invalid) then quit"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 carrier sequential reads: two `read_line`s into the same `[u8; 64]` carrier,
// each echoed -- the second read must overwrite the first line's bytes + length.

#[test]
fn runtime_stdin_line_buffering_carrier_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_STDIN_LINE_BUFFERING_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-line-buffering-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("carrier line buffering canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("carrier line buffering canary should retain its executable receipt");
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("carrier line buffering canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"first\nsecond\n")
        .expect("carrier line buffering input should be written");
    let output = child
        .wait_with_output()
        .expect("carrier line buffering canary should finish");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected carrier line buffering canary to exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "first\nsecond\n",
        "expected each carrier read_line to echo its own line, the second overwriting the first"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 carrier stdin round-trip: `read_line` into a `[u8; 64] in Utf8` carrier
// (stdin straight into the inline bytes + len), then `write_line` the carrier back.

#[test]
fn runtime_text_storage_carrier_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_TEXT_STORAGE);
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-text-storage-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    // Input now fills a fixed range; the caller selects the reported prefix.
    // Exact output below rejects emitting the unused tail. Owner-replacement
    // report markers are not evidence for this contract; raw bounds and untouched
    // tails have separate selected_console_line_reader regressions.
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("fixed-range text input canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("carrier text storage canary should retain its executable receipt");
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("carrier text storage canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"echo me\n")
        .expect("carrier text storage input should be written");
    let output = child
        .wait_with_output()
        .expect("carrier text storage canary should finish");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected carrier text storage canary to exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "> echo me\n",
        "expected the carrier read_line to round-trip the input line back through write_line"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_stderr_write_exit_canary_runs() {
    // The `write_error` host capability mirrors `write` but targets the stderr
    // handle (GetStdHandle(-12)) instead of stdout (-11). The program must emit
    // its text on stderr only, leaving stdout empty, and exit with the requested
    // code.
    let canary = pass_canary(fixture_roster::RUNTIME_STDERR_WRITE_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-stderr-write-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime stderr write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime stderr write canary should retain its executable receipt");
    let output = Command::new(executable)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("runtime stderr write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected runtime stderr write canary to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "hello-stderr\n",
        "expected runtime stderr write canary to emit its text on stderr"
    );
    assert!(
        output.stdout.is_empty(),
        "expected runtime stderr write canary to leave stdout empty, got {:?}",
        String::from_utf8_lossy(&output.stdout)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_stdin_line_buffering_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_STDIN_LINE_BUFFERING_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-stdin-line-buffering-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime stdin line buffering canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime stdin line buffering canary should retain its executable receipt");
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("runtime stdin line buffering canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"hello\nworld\n")
        .expect("stdin line buffering input should be written");
    let output = child
        .wait_with_output()
        .expect("runtime stdin line buffering canary should finish");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected runtime stdin line buffering canary to exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "hello\nworld\n",
        "expected runtime stdin line buffering canary to preserve one logical line per read"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_stdin_crlf_line_read_canary_runs() {
    // Windows terminals (and piped CRLF input) terminate each line with "\r\n".
    // The raw reader retains both bytes through LF. This echo caller explicitly
    // strips LF and preceding CR before write_line; CR alone is not a delimiter.
    // Reuses the two-read echo sample to reject a phantom second empty line.
    let canary = pass_canary(fixture_roster::RUNTIME_STDIN_LINE_BUFFERING_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-stdin-crlf-line-read-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime stdin crlf line read canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime stdin CRLF canary should retain its executable receipt");
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("runtime stdin crlf line read canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"hello\r\nworld\r\n")
        .expect("stdin crlf line read input should be written");
    let output = child
        .wait_with_output()
        .expect("runtime stdin crlf line read canary should finish");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected runtime stdin crlf line read canary to exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "hello\nworld\n",
        "expected CRLF input to read two clean lines (no phantom empty line from the trailing \\n)"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_alias_indexed_string_field_concat_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_ALIAS_INDEXED_STRING_FIELD_CONCAT_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-alias-indexed-string-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime slice alias indexed string field concat canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime slice alias indexed string field concat canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime slice alias indexed string field concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected runtime slice alias indexed string field concat canary to preserve alias-indexed string writes and exit 77, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_indexed_string_guard_exit_canary_runs() {
    // A slice-indexed String field compared against a literal in guard
    // position: an EMPTY (default-zeroed) field takes the false arm, the
    // matching field takes the true arm, and a same-length differing field
    // takes the false arm. Exit 70 only when all three behave (the lying-guard
    // regression took the true arm unconditionally, exiting 71).
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_INDEXED_STRING_GUARD_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-indexed-string-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime slice indexed string guard canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime slice indexed string guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime slice indexed string guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected slice-indexed String guard compares to be content compares (empty != literal, match == literal, same-length differ != literal) and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_machine_indexed_string_guard_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_MACHINE_INDEXED_STRING_GUARD_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-machine-indexed-string-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime slice machine-indexed string guard canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime slice machine-indexed string guard canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime slice machine-indexed string guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(72),
        "expected cross-region slice String writes and guards to preserve exact content and exit 72, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_string_field_literal_guard_exit_canary_runs() {
    // The storage-place sibling of the slice-indexed shape: a machine-owned
    // String field guard-compared against a literal (empty field takes the
    // false arm; written field takes the true arm).
    let canary = pass_canary(fixture_roster::RUNTIME_STRING_FIELD_LITERAL_GUARD_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-string-field-literal-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime string field literal guard canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime string field literal guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime string field literal guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected machine String field guard compares against literals to be content compares and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_local_array_indexed_string_guard_exit_canary_runs() {
    // The frame-BASE-indexed sibling of the slice-indexed shape: a LOCAL
    // inline fixed array's element String field guard-compared against a
    // literal at a runtime index (empty field takes the false arm, matching
    // takes true, same-length-differing takes false; the lying-guard
    // regression selected no compare and took the true arm unconditionally).
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_ARRAY_INDEXED_STRING_GUARD_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-local-array-indexed-string-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime local array indexed string guard canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime local array indexed string guard canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime local array indexed string guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected local-array-indexed String guard compares to be content compares (empty != literal, match == literal, same-length differ != literal) and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_local_array_indexed_string_field_concat_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_ARRAY_INDEXED_STRING_FIELD_CONCAT_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-local-array-indexed-string-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime local-array-indexed string field concat canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime local-array-indexed string field concat canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime local-array-indexed string field concat canary should run");
    assert_eq!(
        output.status.code(),
        Some(89),
        "expected frame-base-indexed text assembly to preserve `prefix omega!` and exit 89, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_fixed_indexed_string_guard_exit_canary_runs() {
    // The CONSTANT-index sibling of the slice-indexed shape: a slice
    // element's String field guard-compared against a literal at a literal
    // index (`room_slice[0]`), lowering through the fixed-indexed place
    // (descriptor deref + folded constant offset). Same three regimes.
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_FIXED_INDEXED_STRING_GUARD_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-fixed-indexed-string-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime slice fixed indexed string guard canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime slice fixed indexed string guard canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime slice fixed indexed string guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected fixed-indexed String guard compares to be content compares (empty != literal, match == literal, same-length differ != literal) and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_pointee_string_guard_exit_canary_runs() {
    // The POINTEE sibling: a String field read through a `&mut Room` pointer
    // slot (local alias AND called-machine parameter), guard-compared against
    // a literal. The pre-fix regression here was an always-unequal compare:
    // the place resolved to the pointer slot's raw bytes rather than the
    // pointee's descriptor, so the MATCH regime took the false arm.
    let canary = pass_canary(fixture_roster::RUNTIME_POINTEE_STRING_GUARD_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-pointee-string-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime pointee string guard canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime pointee string guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime pointee string guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected pointee String guard compares to be content compares (empty != literal, match == literal, same-length differ != literal, parameter shape included) and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_carrier_parameter_concat_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MUTABLE_STRING_PARAMETER_CONCAT_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-carrier-parameter-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime mutable carrier parameter concat canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime mutable carrier parameter concat canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable carrier parameter concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected runtime mutable carrier parameter concat canary to preserve pointee carrier writes and exit 77, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_string_parameter_concat_write_line_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MUTABLE_STRING_PARAMETER_CONCAT_WRITE_LINE);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("runtime mutable carrier parameter concat/write canary should check");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 77);
    assert_eq!(interpreted.stdout, b"prefix omega\n".to_vec());
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-string-parameter-concat-write-line-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime mutable carrier parameter concat write_line canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime mutable carrier parameter concat write_line canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable carrier parameter concat write_line canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected runtime mutable carrier parameter concat write_line canary to print generated pointee text and exit 77, got {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"prefix omega\n");

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_string_parameter_wrapped_concat_write_line_canary_runs() {
    let canary =
        pass_canary(fixture_roster::RUNTIME_MUTABLE_STRING_PARAMETER_WRAPPED_CONCAT_WRITE_LINE);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("runtime wrapped mutable carrier concat/write canary should check");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 77);
    assert_eq!(interpreted.stdout, b"prefix omega done\n".to_vec());
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-string-parameter-wrapped-concat-write-line-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime wrapped mutable carrier concat write_line canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime wrapped mutable carrier concat write_line canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime wrapped mutable carrier concat write_line canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected wrapped mutable carrier concat write_line canary to print generated pointee text and exit 77, got {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"prefix omega done\n");

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_struct_carrier_field_copy_concat_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MUTABLE_STRUCT_STRING_FIELD_COPY_CONCAT_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-struct-carrier-field-copy-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime mutable struct carrier field copy concat canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime mutable struct carrier field copy concat canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable struct carrier field copy concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected runtime mutable struct carrier field copy concat canary to preserve copied carrier fields and exit 77, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}
