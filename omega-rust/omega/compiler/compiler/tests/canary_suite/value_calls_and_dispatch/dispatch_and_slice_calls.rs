use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, Command, compile, compile_reviewed_repository_fixture,
    compile_rooted_canary_for_native_host, executable_name, fs, interpret, pass_canary,
};
use compiler::CheckedCompileRequest;

#[test]
fn runtime_nonentry_inline_second_receiver_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_NONENTRY_INLINE_SECOND_RECEIVER_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-nonentry-inline-second-receiver-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("non-entry inline second-receiver canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("non-entry inline second-receiver canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("non-entry inline second-receiver canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the SECOND receiver through the inline route (7 -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// CALL-BOUND LOCAL TERMINAL through a double-nested second instance:
// the bare value terminal lives in the state's TAIL SEGMENT; the
// return-write's control-flow lookup normalizes to segment 0.

#[test]
fn runtime_nested_local_terminal_second_instance_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_LOCAL_TERMINAL_SECOND_INSTANCE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-nested-local-terminal-second-instance-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested local-terminal canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested local-terminal canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested local-terminal canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the tail-segment local terminal to deliver 6 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// FIELD-BINDING + FIELD-READ TERMINAL through a double-nested SECOND
// instance: the field-binding delivery resolves `self.total` under the
// CALLER's composed receiver base (mid2+8), not the by-type first pick.

#[test]
fn runtime_nested_field_terminal_second_instance_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_FIELD_TERMINAL_SECOND_INSTANCE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-nested-field-terminal-second-instance-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested field-terminal canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested field-terminal canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested field-terminal canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the second Mid's field delivery (12 -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// MULTI-ARM inline callee with SAME-NAMED arm locals (the account_ledger
// regression shape): each arm's `b` must resolve in THAT arm's scope; a
// call-target-scoped key stole every arm's delivery for arm 0's slot.

#[test]
fn runtime_multiarm_same_named_locals_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MULTIARM_SAME_NAMED_LOCALS_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-multiarm-same-named-locals-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("multi-arm same-named locals canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("multi-arm same-named locals canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("multi-arm same-named locals canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected per-arm local deliveries (10/20/30 -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// MULTI-ARM inline callee with TEXTEQ-valued arm locals (`let b: bool =
// self.name == "omega"` per non-leaf sub-state arm): the arm bodies have no
// other emission route on the flattened leaf walk, so their call-free
// LocalData initializers must ride the Terminal-value expansions and write
// BEFORE the terminal copy (hit==true, miss==false -> exit 70).

#[test]
fn runtime_multiarm_texteq_local_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MULTIARM_TEXTEQ_LOCAL_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("multi-arm carrier text-equality local canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should preserve both multi-arm carrier comparisons (exit 70), got {}",
        outcome.exit_code
    );
    let build_dir = std::env::temp_dir().join(format!(
        "omega-multiarm-texteq-local-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("multi-arm texteq locals canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("multi-arm texteq locals canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("multi-arm texteq locals canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected per-arm texteq deliveries (hit true / miss false -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// A selected sub-state's call-free LocalData runs in its straight-line prelude,
// before that sub-state's nested guard. Text equality needs the dedicated
// frame-slot comparison writer on this path; otherwise the guard reads ZII.

#[test]
fn runtime_pre_guard_texteq_local_guard_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_PRE_GUARD_TEXTEQ_LOCAL_GUARD_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("pre-guard carrier text-equality canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should observe the initialized carrier comparison (exit 70), got {}",
        outcome.exit_code
    );
    let build_dir = std::env::temp_dir().join(format!(
        "omega-pre-guard-texteq-local-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("pre-guard texteq local guard canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("pre-guard texteq local guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("pre-guard texteq local guard canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the nested guard to read the initialized texteq local (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// The same pre-guard value must exist before nested transition-argument
// capture, not only before a guard read.

#[test]
fn runtime_pre_guard_texteq_local_arg_forward_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_PRE_GUARD_TEXTEQ_LOCAL_ARG_FORWARD_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("forwarded carrier text-equality local canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should forward the initialized carrier comparison (exit 70), got {}",
        outcome.exit_code
    );
    let build_dir = std::env::temp_dir().join(format!(
        "omega-pre-guard-texteq-local-arg-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("pre-guard texteq local argument canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("pre-guard texteq local argument canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("pre-guard texteq local argument canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected argument capture to forward the initialized texteq local (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// PARAM-BINDING SERVE: a spliced helper's `&mut Tally` param receiver
// delivers on the PASSED instance (the second of two) -- the receiver
// chain walk binds the param to its argument's base at each descent.

#[test]
fn runtime_param_receiver_second_instance_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_PARAM_RECEIVER_SECOND_INSTANCE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-param-receiver-second-instance-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("second-instance param receiver canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("second-instance param receiver canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("second-instance param receiver canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the param binding to deliver second's 9 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// PARAM FORWARDING CHAIN through a re-borrow (`self.inner(&mut t)` where
// t is itself `&mut Tally`): the walk's env forwards the binding; the
// interp collapses re-borrow Ref nesting (was an "unknown value-call
// target" decline).

#[test]
fn runtime_param_forward_chain_second_receiver_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_PARAM_FORWARD_CHAIN_SECOND_RECEIVER_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-param-forward-chain-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("param forward-chain canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("param forward-chain canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("param forward-chain canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the forwarded param to deliver second's 9 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// BUILD-MACHINE IDENTITY IS FILE-BASED: a `Maker::build(b: &mut Build)`
// in MAIN source stays an ordinary runtime machine (the build hook must
// be declared at a build.omg root).

#[test]
fn runtime_main_source_builder_is_ordinary_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MAIN_SOURCE_BUILDER_IS_ORDINARY_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-main-source-builder-ordinary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("main-source builder canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("main-source builder canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("main-source builder canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the ordinary builder to run at RUNTIME (7 -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// std::time SATURATING twins: Instant/SystemTime saturating_add/subtract
// clamp to the new MAX/EPOCH/MIN consts; seven exact legs (D14 fire-F
// equality guards pin the u64::MAX / i64 extreme values).

#[test]
fn runtime_saturating_time_arith_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SATURATING_TIME_ARITH_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-saturating-time-arith-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("saturating time arithmetic canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("saturating time arithmetic canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected all seven saturation legs exact (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// NATURAL TERMINATION exits 0, matching the interpreter oracle (native
// returned register garbage before the terminate-edge zeroing).

#[test]
fn runtime_natural_termination_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_NATURAL_TERMINATION_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-natural-termination-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("natural termination canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("natural termination canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("natural termination canary should run");
    assert_eq!(
        output.status.code(),
        Some(0),
        "natural termination must exit 0 like the oracle, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// DEEP-STATE NAME COLLISION: a deep arm's arg delivers past a live
// same-named entry local (the receiver epic's last theoretical residual,
// probed not-reproducible -- this pin keeps it that way).

#[test]
fn runtime_deep_state_name_collision_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DEEP_STATE_NAME_COLLISION_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-deep-state-name-collision-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("deep-state name collision canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("deep-state name collision canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("deep-state name collision canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the DEEP arm's v (9 -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// D14 FIRES E+F: u64::MAX literals in a LET initializer and an EQUALITY
// guard round-trip exactly through a value machine's guarded arms.

#[test]
fn runtime_u64_literal_let_guard_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_U64_LITERAL_LET_GUARD_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-u64-literal-let-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("u64 let+guard canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("u64 let+guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("u64 let+guard canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the exact u64::MAX round trip (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// PARAM RECEIVER through a SINGLE-instance family: the by-type pick is
// provably the passed instance (multi-instance serves via param binding;
// unresolvable-argument shapes stay fenced).

#[test]
fn runtime_param_receiver_single_instance_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_PARAM_RECEIVER_SINGLE_INSTANCE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-param-receiver-single-instance-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("single-instance param receiver canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("single-instance param receiver canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("single-instance param receiver canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the param receiver's delivery (9 -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// A dispatched value call whose terminal reads THROUGH a `&mut` ALIAS
// (`-> acc`, acc: &mut i32): pins that the result is the pointee value,
// never the pointer bits (the last unprobed return-write shape).

#[test]
fn runtime_dispatch_result_alias_read_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DISPATCH_RESULT_ALIAS_READ_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-dispatch-result-alias-read-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dispatch alias-read terminal canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dispatch alias-read terminal canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dispatch alias-read terminal canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the dispatched call's `-> acc` alias terminal to deliver the \
         pointee 63 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A dispatched value call whose terminal reads a SLICE ELEMENT directly
// (`-> s[j]`): the return-write emits the region-paired indexed copy
// (frame slot -> CopyRuntimeFrameIndexedToRuntimeFrame). The first probe
// emitted the machine-region kind against the frame slot and crashed;
// the region split is the fix (2026-07-09k2).

#[test]
fn runtime_dispatch_slice_element_terminal_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DISPATCH_SLICE_ELEMENT_TERMINAL_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-dispatch-slice-element-terminal-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dispatch slice-element terminal canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dispatch slice-element terminal canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dispatch slice-element terminal canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the dispatched call's `-> s[j]` terminal to deliver s[2] == 7 \
         (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A dispatched value call whose terminal is a BINARY expression (-> acc + 100): computed into the result place (was a silent fallthrough).

#[test]
fn runtime_dispatch_result_binary_terminal_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DISPATCH_RESULT_BINARY_TERMINAL_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-dispatch-result-binary-terminal-exit-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime_dispatch_result_binary_terminal_exit should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("binary-terminal canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime_dispatch_result_binary_terminal_exit should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the binary terminal to deliver (n == 105 -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Multi-arm terminals (place arm + binary arm) at two call sites taking opposite arms, field-bound results.

#[test]
fn runtime_dispatch_result_multi_arm_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DISPATCH_RESULT_MULTI_ARM_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-dispatch-result-multi-arm-exit-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime_dispatch_result_multi_arm_exit should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("multi-arm result canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime_dispatch_result_multi_arm_exit should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected both arms' terminals to deliver (high == 8, low == -4 -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A dispatched value call as a GUARD SUBJECT: the hoist temp's result slot is served by the return-write.

#[test]
fn runtime_dispatch_result_guard_subject_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DISPATCH_RESULT_GUARD_SUBJECT_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-dispatch-result-guard-subject-exit-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime_dispatch_result_guard_subject_exit should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("guard-subject result canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime_dispatch_result_guard_subject_exit should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the guard-subject call result to deliver (== 9 -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A dispatched call result consumed DIRECTLY as a transition argument
// (true -> check(self.count(..))): argument materialization descends into
// transition expressions, the clone terminal stamps CallResultReturn from
// the plan role, and the return-write keys on the return-target dispatch.

#[test]
fn runtime_dispatch_result_transition_arg_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DISPATCH_RESULT_TRANSITION_ARG_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-dispatch-result-transition-arg-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dispatch result transition-arg canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dispatch result transition-arg canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dispatch result transition-arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the direct transition-arg call result to deliver \
         (n == 12 -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The fence-exemption acceptance test: an EFFECTFUL re-entrant value callee
// dispatches and delivers both the looped result and the per-entry effect
// count (the 2026-07-08n retraction counterexample, now sound).

#[test]
fn runtime_dispatched_effectful_reentrant_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DISPATCHED_EFFECTFUL_REENTRANT_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-dispatched-effectful-reentrant-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dispatched effectful re-entrant canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dispatched effectful re-entrant canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dispatched effectful re-entrant canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected total == 5 AND hits == 5 (exit 70; 71 = result wrong, 72 = \
         effect count wrong), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A dispatched terminal constructing an ENUM CASE with a payload -- the
// wrapper-result shape (zero slot, tag, payload fields at variant offsets).

#[test]
fn runtime_dispatch_result_enum_case_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DISPATCH_RESULT_ENUM_CASE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-dispatch-result-enum-case-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dispatch enum-case result canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dispatch enum-case result canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dispatch enum-case result canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Verdict::Yes {{ score: 15 }} to deliver tag+payload (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A MACHINE ARRAY as a slice argument to a dispatched call: the descriptor
// arm writes {ptr = base+offset, len} (raw-bytes-as-pointer was a SIGSEGV).

#[test]
fn runtime_dispatch_machine_array_slice_arg_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DISPATCH_MACHINE_ARRAY_SLICE_ARG_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-dispatch-machine-array-slice-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("machine-array slice-arg canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("machine-array slice-arg canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("machine-array slice-arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected nums[2] == 7 through the dispatched slice (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A dispatched value call whose TERMINAL returns a FIELD read: the
// return-write copy uses the resolved place's REGION (was hardcoded
// RuntimeFrame, reading the frame at a machine offset -- garbage).

#[test]
fn runtime_dispatch_result_field_terminal_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DISPATCH_RESULT_FIELD_TERMINAL_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-dispatch-result-field-terminal-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dispatch result field-terminal canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dispatch result field-terminal canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dispatch result field-terminal canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the dispatched call's field-read terminal to deliver \
         (n == 42 -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_called_machine_loop_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_CALLED_MACHINE_LOOP_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-nested-called-machine-loop-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime nested called machine loop canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime nested called machine loop canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a loop nested two calls deep (Main -> Helper::run -> Lookup::search -> \
         find_at) to specialize the whole call chain and thread main's continuation down \
         through the tail calls, exiting 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_state_loop_indexed_search_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_STATE_LOOP_INDEXED_SEARCH_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-state-loop-indexed-search-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime state loop indexed search canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime state loop indexed search canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a self-looping state (dispatch back-edge) that searches a slice by a \
         loop-carried index and passes the found element's field to a successor state to \
         exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_call_result_through_reference_field_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CALL_RESULT_THROUGH_REFERENCE_FIELD_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-call-result-through-reference-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime call result through reference field canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("call result through reference field canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime call result through reference field canary should run");

    assert_eq!(
        output.status.code(),
        Some(183),
        "expected a machine-call result assigned through a reference field \
         (`ref.field = self.call()`) to write through the pointer once, not also \
         clobber the reference slot, and exit 183, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_string_call_result_through_reference_field_exit_canary_runs() {
    let canary =
        pass_canary(fixture_roster::RUNTIME_STRING_CALL_RESULT_THROUGH_REFERENCE_FIELD_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-string-call-result-through-reference-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime string call result through reference field canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "string result through reference field canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime string call result through reference field canary should run");

    assert_eq!(
        output.status.code(),
        Some(186),
        "expected a string machine-call result assigned through a reference field \
         (`ref.label = self.call()`) to copy the returned string descriptor through \
         the pointer and exit 186, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_two_string_call_results_through_reference_fields_exit_canary_runs() {
    let canary =
        pass_canary(fixture_roster::RUNTIME_TWO_STRING_CALL_RESULTS_THROUGH_REFERENCE_FIELDS_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-two-string-call-results-through-reference-fields-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime two string call results through reference fields canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "two string results through reference fields canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime two string call results through reference fields canary should run");

    assert_eq!(
        output.status.code(),
        Some(194),
        "expected two string call results assigned through reference fields to preserve both descriptors and exit 194, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_offset_string_call_results_through_reference_fields_exit_canary_runs() {
    let canary = pass_canary(
        fixture_roster::RUNTIME_OFFSET_STRING_CALL_RESULTS_THROUGH_REFERENCE_FIELDS_EXIT,
    );
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-offset-string-call-results-through-reference-fields-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime offset string call results through reference fields canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "offset string results through reference fields canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime offset string call results through reference fields canary should run");

    assert_eq!(
        output.status.code(),
        Some(196),
        "expected string call results assigned through +16/+32 reference fields to preserve both descriptors and exit 196, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_reference_returned_slice_element_write_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_REFERENCE_RETURNED_SLICE_ELEMENT_WRITE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-reference-returned-slice-element-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime reference returned slice element write canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "reference-returned slice-element write canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime reference returned slice element write canary should run");

    assert_eq!(
        output.status.code(),
        Some(181),
        "expected a machine returning `&mut slice[index]` to bind the element address \
         (not copy the referent) so writes through the reference land, and exit 181, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_reference_returned_slice_element_through_param_exit_canary_runs() {
    let canary =
        pass_canary(fixture_roster::RUNTIME_REFERENCE_RETURNED_SLICE_ELEMENT_THROUGH_PARAM_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-reference-returned-slice-element-through-param-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("reference-returning called machine canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("reference-returning parameter canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("reference-returning called machine canary should run");

    // The called machine `pick` returns `&mut cells[2]`; its `let cells = ...
    // as_mut_slice()` descriptor init must be materialised, otherwise the returned
    // address is computed from an uninitialized descriptor and the write segfaults.
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a called machine returning `&mut slice[index]` to materialise its \
         slice-descriptor local and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_guarded_reference_returned_slice_element_exit_canary_runs() {
    let canary =
        pass_canary(fixture_roster::RUNTIME_NESTED_GUARDED_REFERENCE_RETURNED_SLICE_ELEMENT_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-nested-guarded-reference-returned-slice-element-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested guarded reference-returning called machine canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("nested guarded reference-returning canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested guarded reference-returning called machine canary should run");

    assert_eq!(
        output.status.code(),
        Some(184),
        "expected a nested guarded call returning `&mut slice[index]` to materialise \
         the returned reference slot before the caller writes through it, and exit 184, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_local_indexed_parameter_write_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MUTABLE_LOCAL_INDEXED_PARAMETER_WRITE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-local-indexed-parameter-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime mutable local indexed parameter write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("mutable local indexed parameter canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable local indexed parameter write canary should run");

    assert_eq!(
        output.status.code(),
        Some(171),
        "expected runtime mutable local indexed parameter write canary to preserve writes through local fixed-array indexed mutable call parameters and exit 171, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_machine_owned_local_indexed_parameter_write_exit_canary_runs() {
    let canary = pass_canary(
        fixture_roster::RUNTIME_MUTABLE_MACHINE_OWNED_LOCAL_INDEXED_PARAMETER_WRITE_EXIT,
    );
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-machine-owned-local-indexed-parameter-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime mutable machine-owned local indexed parameter write canary should compile",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "machine-owned local indexed parameter canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable machine-owned local indexed parameter write canary should run");

    assert_eq!(
        output.status.code(),
        Some(173),
        "expected runtime mutable machine-owned local indexed parameter write canary to preserve writes through machine-owned collection + local indexed mutable call parameters and exit 173, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_dynamic_indexed_machine_owned_parameter_write_exit_canary_runs() {
    let canary = pass_canary(
        fixture_roster::RUNTIME_MUTABLE_DYNAMIC_INDEXED_MACHINE_OWNED_PARAMETER_WRITE_EXIT,
    );
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-dynamic-indexed-machine-owned-parameter-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime mutable dynamic indexed machine-owned parameter write canary should compile",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "dynamic machine-owned indexed parameter canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable dynamic indexed machine-owned parameter write canary should run");

    assert_eq!(
        output.status.code(),
        Some(175),
        "expected runtime mutable dynamic indexed machine-owned parameter write canary to preserve writes through machine-owned collection + dynamic indexed mutable call parameters and exit 175, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dispatch_local_index_binary_write_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DISPATCH_LOCAL_INDEX_BINARY_WRITE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-local-index-binary-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime local index binary write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("local index binary write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime local index binary write canary should run");

    assert_eq!(
        output.status.code(),
        Some(191),
        "expected runtime local index binary write canary to preserve direct caller-local indexed binary writes and exit 191, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dispatch_helper_local_alias_add_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DISPATCH_HELPER_LOCAL_ALIAS_ADD_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-dispatch-helper-local-alias-add-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime dispatch helper local alias add canary should compile from its authored root",
    );

    let executable = compilation
        .checked_native_executable_path()
        .expect("helper local alias-add canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime dispatch helper local alias add canary should run");

    assert_eq!(
        output.status.code(),
        Some(181),
        "expected runtime dispatch helper local alias add canary to preserve append_exit mutation through local slice alias and exit 181, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_alias_indexed_field_write_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_ALIAS_INDEXED_FIELD_WRITE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-alias-indexed-field-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime slice alias indexed field write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("slice alias indexed field write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime slice alias indexed field write canary should run");

    assert_eq!(
        output.status.code(),
        Some(201),
        "expected runtime slice alias indexed field write canary to write through a local slice alias and exit 201, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_indexed_binary_rmw_exit_canary_runs() {
    // The runtime twin of requires_slice_indexed_alias_field_binary_compile:
    // a binary RMW through a slice-descriptor alias with a runtime index
    // lowers to WriteRuntimeFrameIndexedBinary, whose x86_64 encoding landed
    // 2026-07-18 (aarch64-only from birth; the compile canary refused with
    // the zero-layout-width error on x86_64 hosts). exit 71 = the RMW missed
    // the element.
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_INDEXED_BINARY_RMW_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("slice indexed binary RMW canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (element 2 bumped 30 -> 31), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-slice-indexed-binary-rmw-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("slice indexed binary RMW canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("slice indexed binary RMW canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("slice indexed binary RMW canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the descriptor-indexed binary RMW to land (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mut_ref_forward_exit_canary_runs() {
    // The LEGAL shape the borrow-mutability check must keep accepting: a
    // `&mut` param forwarded by BARE NAME to another `&mut` param (a Name,
    // not a `&mut` node -- a syntactic check would false-positive). The
    // callee writes through the double-hopped reference; the caller
    // observes it via the aliased field. exit 71 = the forwarded write
    // missed self.c.
    let canary = pass_canary(fixture_roster::RUNTIME_MUT_REF_FORWARD_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("mut-ref forward canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (forwarded write lands), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-mut-ref-forward-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("mut-ref forward canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("mut-ref forward canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("mut-ref forward canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the bare-name `&mut` forward to stay legal and deliver (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_local_slice_forward_exit_canary_runs() {
    // A frame-LOCAL-backed `&mut [T]` descriptor (view of a struct-literal
    // local's array field) forwarded as a transition arg, then indexed-RMW'd
    // through the param. Exit 71 = the RMW read the wrong initial value;
    // a SIGNAL death = the descriptor went ZII/wild (the promoted segfault).
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_SLICE_FORWARD_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("local-slice forward canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (forwarded slice RMW lands), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-local-slice-forward-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("local-slice forward canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("local-slice forward canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("local-slice forward canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the forwarded local-backed slice descriptor to stay live (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn f32_guard_const_arith_landed_exit_canary_runs() {
    // F2c: the constant guard tree folds/evaluates per-op at the f32 landed
    // width on BOTH engines (2^24 + 1.0 == 2^24 at f32; an f64 window says
    // 16777217.0 and takes the wrong arm).
    let canary = pass_canary(fixture_roster::F32_GUARD_CONST_ARITH_LANDED_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("f32 guard const-arith canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter should fold the guard tree per-op at f32 (exit 70), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-f32-guard-landed-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("f32 guard const-arith canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("f32 guard const-arith canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the native guard fold at the f32 landed width (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn f32_arg_const_arith_landed_exit_canary_runs() {
    // F2c ARG face: a wholly anonymous constant tree remains exact Rat until
    // the transition parameter requests f32, then rounds once to 1 + 2^-23 on
    // BOTH engines. Explicitly landed/runtime trees remain per-op elsewhere.
    let canary = pass_canary(fixture_roster::F32_ARG_CONST_ARITH_LANDED_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("f32 arg const-arith canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter should exact-fold then land the arg tree once at f32 (exit 70), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-f32-arg-landed-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("f32 arg const-arith canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("f32 arg const-arith canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the native arg tree at the f32 landed width (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}
