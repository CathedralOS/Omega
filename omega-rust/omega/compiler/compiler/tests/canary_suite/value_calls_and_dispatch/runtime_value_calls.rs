use super::assert_native_exit_code;
use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, Command, Stdio, compile,
    compile_reviewed_repository_fixture,
    compile_rooted_backend_canary_without_output_for_target_and_permission_policy,
    compile_rooted_canary_for_native_host, compile_rooted_canary_for_target, fs,
    hosted_main_program_entry_build_for, interpret, pass_canary,
};
use checked_interpreter::BuildMachineEntry;
use compiler::CheckedCompileRequest;
use std::io::Write;

#[test]
fn runtime_indexed_copy_aggregate_handoff_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_COPY_AGGREGATE_HANDOFF_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("runtime-indexed copy-aggregate handoff should reach checked trees");
    let stdin = [5, 27, 33, 44, b'\n'];
    let interpreted = interpret(&checked, &stdin);
    assert_eq!(
        interpreted.error, None,
        "reference execution should succeed"
    );
    assert_eq!(
        interpreted.exit_code, 70,
        "reference execution should retain every field"
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-indexed-copy-aggregate-handoff-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime-indexed copy-aggregate handoff should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime-indexed copy-aggregate handoff should retain its executable receipt");
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .spawn()
        .expect("runtime-indexed copy-aggregate handoff should run");
    child
        .stdin
        .as_mut()
        .expect("stdin pipe should exist")
        .write_all(&stdin)
        .expect("runtime input should be written");
    let output = child
        .wait_with_output()
        .expect("native execution should finish");
    assert_eq!(
        output.status.code(),
        Some(70),
        "the argument and return handoffs should retain every selected field; got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_call_before_transition_args_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MUTABLE_CALL_BEFORE_TRANSITION_ARGS_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("mutable-call statement-order canary should reach checked trees");
    let stdin = [5, 27, 33, 44, b'\n'];
    let interpreted = interpret(&checked, &stdin);
    assert_eq!(
        interpreted.error, None,
        "reference execution should succeed"
    );
    assert_eq!(
        interpreted.exit_code, 70,
        "reference execution should observe the call writes"
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-call-before-transition-args-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("mutable-call statement-order canary should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("mutable-call statement-order canary should retain its executable receipt");
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .spawn()
        .expect("mutable-call statement-order canary should run");
    child
        .stdin
        .as_mut()
        .expect("stdin pipe should exist")
        .write_all(&stdin)
        .expect("runtime input should be written");
    let output = child
        .wait_with_output()
        .expect("native execution should finish");
    assert_eq!(
        output.status.code(),
        Some(70),
        "transition args should observe scalar writes from the preceding statement call; got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_referenced_local_outlives_sibling_guard_call_exit_canary_runs() {
    let canary =
        pass_canary(fixture_roster::RUNTIME_REFERENCED_LOCAL_OUTLIVES_SIBLING_GUARD_CALL_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("referenced-local sibling-guard canary should compile to checked trees");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None, "should interpret cleanly");
    assert_eq!(
        interpreted.exit_code, 70,
        "interpreter must observe the nested hall mutation before the outer result guard"
    );
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-referenced-local-outlives-sibling-guard-call-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("referenced-local-outlives-sibling-guard-call canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "referenced-local sibling-guard canary",
        "a `&mut local` pointee should survive its sibling value-call guard chain",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_view_linked_input_unrelated_ref_write_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_VIEW_LINKED_INPUT_UNRELATED_REF_WRITE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-view-linked-input-unrelated-ref-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("view-linked-input-unrelated-ref-write canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "view-linked-input unrelated-ref-write canary",
        "an elision-linked view of `a` should coexist with the write to unlinked `b`",
    );

    let _ = fs::remove_dir_all(&build_dir);

    let cross_dir = std::env::temp_dir().join(format!(
        "omega-runtime-view-linked-input-unrelated-ref-write-linux-x64-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&cross_dir);
    compile_rooted_canary_for_target(&canary, cross_dir.join("out"), "linux_x86_64")
        .expect("view-linked-input aggregate write should cross-compile for linux_x64");
    let _ = fs::remove_dir_all(&cross_dir);
}

#[test]
fn runtime_value_call_single_execution_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_SINGLE_EXECUTION_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-value-call-single-execution-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("value-call single-execution canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "value-call single-execution canary",
        "each written value call should execute exactly once",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_explicit_discard_executes_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_EXPLICIT_DISCARD_EXECUTES_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-explicit-discard-executes-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("explicit-discard canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "explicit-discard single-execution canary",
        "an explicitly discarded value call should execute exactly once",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_transition_subject_call_single_evaluation_exit_canary_runs() {
    let canary =
        pass_canary(fixture_roster::RUNTIME_TRANSITION_SUBJECT_CALL_SINGLE_EVALUATION_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-transition-subject-call-single-evaluation-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("transition-subject single-evaluation canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "transition-subject single-evaluation canary",
        "a transition guard subject call should execute exactly once",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nonplace_record_pattern_single_evaluation_exit_canary_runs() {
    let canary =
        pass_canary(fixture_roster::RUNTIME_NONPLACE_RECORD_PATTERN_SINGLE_EVALUATION_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-nonplace-record-pattern-single-evaluation-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("computed record-pattern subject canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "computed record-pattern subject canary",
        "one computed-subject call should feed both captured Point field reads",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_effectful_subject_single_evaluation_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_EFFECTFUL_SUBJECT_SINGLE_EVALUATION_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-effectful-subject-single-evaluation-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("effectful-subject single-evaluation canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "effectful transition-subject canary",
        "a diverging-arm transition's nested effectful subject should execute exactly once",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_statement_call_single_execution_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_STATEMENT_CALL_SINGLE_EXECUTION_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-statement-call-single-execution-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("statement-call single-execution canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "statement-call single-execution canary",
        "a statement-position call chain's leaf side effect should execute exactly once",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_assignment_call_post_mutation_value_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_ASSIGNMENT_CALL_POST_MUTATION_VALUE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-assignment-call-post-mutation-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("assignment-call post-mutation value canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "assignment-call post-mutation value canary",
        "the assignment call should deliver its post-mutation value",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_value_call_return_types_exit_canary_runs() {
    // Value-returning calls across return types (i32 / struct / enum / bool) + the
    // un-nested nested-call pattern. Locks the working value-call core. (A value-call
    // written directly as an arg to another VALUE-call miscompiles -- tracked
    // separately; the sound form is to bind the inner call to a local first.)
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_RETURN_TYPES_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-value-call-return-types-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("value-call return-types canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "value-call return-types canary",
        "value calls returning i32, struct, enum, and bool should self-check",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_value_call_struct_result_to_target_exit_canary_runs() {
    // Delivering a value-call STRUCT result: dispatch scalar -> field, bare-body struct
    // -> field, and dispatch struct -> local -> field all work. (A dispatch-body value-
    // call returning a struct assigned DIRECTLY to a field silently stores 0 -- tracked
    // separately; bind to a local first.)
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_STRUCT_RESULT_TO_TARGET_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-value-call-struct-result-target-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("value-call struct-result-to-target canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "value-call struct-result-to-target canary",
        "value-call struct-result delivery and its local workaround should self-check",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_value_call_self_field_enum_match_exit_canary_runs() {
    // A value-call dispatching on an ENUM FIELD of self (`transition self.s { .. }`),
    // called twice with different field values to prove real dispatch. (A method on the
    // enum TYPE matching bare `self`, called `self.s.sides()`, mis-dispatches -- tracked
    // separately; dispatching on a self field or a param both work.)
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_SELF_FIELD_ENUM_MATCH_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-value-call-self-field-enum-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("value-call self-field-enum-match canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "value-call self-field enum-match canary",
        "a value call dispatching on a self enum field should self-check",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_value_call_struct_literal_arms_exit_canary_runs() {
    // A value-call whose transition arms return STRUCT / enum-CASE literals
    // (`transition d { Dir::E -> Vec2 { dx: 1, dy: 0 } ... }`). This was a parse error
    // (a struct-literal arm value is name-like, so the target parser read only the
    // leading path and left the `{`); fixed by re-parsing a path-followed-by-`{` arm
    // value as an expression. The natural "dispatch on an enum, return a struct" shape.
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_STRUCT_LITERAL_ARMS_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-value-call-struct-lit-arms-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("value-call struct-literal-arms canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "value-call struct-literal-arms canary",
        "a value call returning struct and case literals from its arms should self-check",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_contained_machine_exit_canary_runs() {
    // A contained machine (component with state): single-instance method calls --
    // statement-call mutation, arg, and a value-call return -- all work. (Multiple
    // contained machines of the SAME type alias to the first; tracked separately.)
    let canary = pass_canary(fixture_roster::RUNTIME_CONTAINED_MACHINE_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-contained-machine-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("contained-machine canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "contained-machine canary",
        "contained-machine increment, add_to, and get calls should self-check",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_call_result_after_splice_mutation_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CALL_RESULT_AFTER_SPLICE_MUTATION_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-call-result-after-splice-mutation-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("call-result-after-splice-mutation canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "call-result-after-splice-mutation canary",
        "a consumer of the call result should receive the post-mutation value",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_called_machine_loop_search_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CALLED_MACHINE_LOOP_SEARCH_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-called-machine-loop-search-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime called machine loop search canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "called-machine loop-search canary",
        "a cyclic called-machine state should lower as a dispatch back-edge",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_trailing_local_return_exit_canary_runs() {
    // A machine whose trailing terminal expression is a BARE LOCAL NAME must
    // return that local's value, captured at its declaration. The storage
    // planner did not count a trailing `expression` statement as a reference
    // that requires storage, so the local had no frame slot, the bare name
    // could not resolve as a place at selection, and the call-result write
    // silently dropped (`let r = f()` left r at 0). The canary pins three
    // shapes: capture-before-field-mutation (must deliver the CAPTURED value,
    // not the post-mutation re-read), computed-from-param, and a free machine
    // returning a literal-folded local.
    let canary = pass_canary(fixture_roster::RUNTIME_TRAILING_LOCAL_RETURN_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-trailing-local-return-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime trailing local return canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("trailing local return canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime trailing local return canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected every trailing-bare-local return to deliver its declaration-time value \
         (exit 70); 71 = capture-before-mutation returned wrong/zero, 72 = param-computed \
         local wrong, 73 = free-machine literal local wrong. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_looping_value_return_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_LOOPING_VALUE_RETURN_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-recursive-value-return-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime recursive value return canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "looping value-return canary",
        "a looping value call should write its terminal value into the caller result slot",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_looping_cast_return_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_LOOPING_CAST_RETURN_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-looping-cast-return-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime looping cast return canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "looping cast-return canary",
        "the dispatched u8 accumulator should widen into the i32 caller slot",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_value_call_slice_len_guard_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_SLICE_LEN_GUARD_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-value-call-slice-len-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime value call slice len guard canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "value-call slice-length guard canary",
        "an inline value-call guard should observe the elided caller slice's static length",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_sleep_exit_canary_runs() {
    // Clock.sleep uses the selected target's hosted millisecond-sleep realization.
    // Reaching exit_process(70) proves its immediate and field arguments survive
    // the selected native ABI and both non-terminal calls return cleanly.
    let canary = pass_canary(fixture_roster::RUNTIME_SLEEP_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-sleep-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("sleep canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "sleep canary",
        "immediate and field-duration sleep calls should return before exit_process",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_write_no_newline_exit_canary_runs() {
    // `write` (Stdout, no trailing newline) vs `write_line`. The differential oracle
    // checks the exact stdout ("ABC\n"); this run-test asserts that same byte-exact
    // contract natively alongside the expected exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_WRITE_NO_NEWLINE_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-write-no-newline-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("write-no-newline canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .unwrap_or_else(|| {
            panic!("write-no-newline canary lost its exact executable publication receipt")
        });
    let output = Command::new(executable)
        .output()
        .unwrap_or_else(|error| panic!("write-no-newline canary should run: {error}"));
    assert_eq!(
        output.stdout,
        b"ABC\n",
        "write followed by write_line must emit exactly ABC\\n natively; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.status.code(),
        Some(70),
        "write followed by write_line should reach the expected exit; got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_exit_code_exit_canary_runs() {
    // `exit_process(self.v)` with a RUNTIME (non-constant) i32 must exit with the
    // computed value. Regression guard for the documented footgun where a runtime
    // exit-code operand was ignored and the process silently exited 0. The canary
    // computes 5 + 65 = 70 and exits with `self.v`.
    let canary = pass_canary(fixture_roster::RUNTIME_EXIT_CODE_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-exit-code-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime exit code canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "runtime exit-code canary",
        "exit_process should consume the computed runtime i32 value",
    );

    let _ = fs::remove_dir_all(&build_dir);

    for target in ["windows_x86_64", "macos_arm64"] {
        let cross_dir = std::env::temp_dir().join(format!(
            "omega-runtime-exit-code-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&cross_dir);
        let source_dir = cross_dir.join("src");
        let output_dir = cross_dir.join("out");
        fs::create_dir_all(&source_dir).expect("runtime exit-code cross-target source directory");
        fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy runtime exit-code canary");
        fs::write(
            source_dir.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write runtime exit-code cross-build source");
        compile(CanaryCompileSpec {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(output_dir.clone()),
            target_name: Some(target.to_owned()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("runtime exit-code cross-compile failed for {target}: {diagnostics:#?}")
        });
        let footprints = fs::read_to_string(output_dir.join("08_boundary_footprints.json"))
            .expect("runtime exit-code cross-target footprints should be emitted");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_outbound_storage_import\""),
            "{target} runtime exit import must retain its exact storage-call footprint"
        );
        let _ = fs::remove_dir_all(&cross_dir);
    }
}

#[test]
fn borrow_carrying_data_field_exit_canary_runs() {
    // Borrow-carrying data (decision 15 stage 2/3): constructing `Msg { body:
    // &self.cell }` and reading the reference field `message.body` extracts the
    // borrowed `&Cell`, which is dereferenced through a `&Cell` ref parameter.
    // Both the interpreter oracle AND the native backend must exit 70 (a 0/71
    // exit is the pre-fix bug where a struct-literal-rooted field read resolved
    // to no place and left the target zero).
    let canary = pass_canary(fixture_roster::BORROW_CARRYING_DATA_FIELD_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("borrow-carrying data canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should read the borrowed field as 70, got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-borrow-carrying-data-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("borrow-carrying data canary should compile to a PE");
    assert_native_exit_code(
        &compilation,
        70,
        "borrow-carrying data-field canary",
        "the native backend should read the borrowed field like the interpreter",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_u8_field_arith_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_U8_FIELD_ARITH_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-u8-field-arith-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("u8 field arithmetic canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "u8 field-arithmetic canary",
        "u8 fields should store, add, and compare as one-byte values",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_i8_signed_arith_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_I8_SIGNED_ARITH_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-i8-signed-arith-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("i8 signed arithmetic canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "i8 signed-arithmetic canary",
        "i8 fields should preserve signed one-byte arithmetic",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_i16_signed_arith_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_I16_SIGNED_ARITH_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-i16-signed-arith-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("i16 signed arithmetic canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("i16 signed arithmetic canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("i16 signed arithmetic canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i16 fields to be SIGNED 2-byte values (-1000+400==-600, then -600<0 \
         via a signed 16-bit guard compare; an unsigned or 1-byte treatment exits 71), \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_u16_field_arith_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_U16_FIELD_ARITH_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-u16-field-arith-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("u16 field arithmetic canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("u16 field arithmetic canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("u16 field arithmetic canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected u16 fields to store/add/compare as 2-byte UNSIGNED values \
         (40000+30000 wraps to 4464; 40000>30000 needs an unsigned 16-bit compare), \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_addr_field_exit_canary_runs() {
    // `addr` is a pointer-width ADDRESS type (distinct from usize/counts). Store
    // two distinct addresses in struct fields (the UEFI EfiHandle/ConsolePtr
    // shape), read one back via `.raw`, cast to i32: exit 88.
    let canary = pass_canary(fixture_roster::RUNTIME_ADDR_FIELD_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-addr-field-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("addr field canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("addr field canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("addr field canary should run");

    assert_eq!(
        output.status.code(),
        Some(88),
        "expected addr field round-trip (ConsolePtr.raw = 88, exit 88), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_i64_signed_arith_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_I64_SIGNED_ARITH_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-isize-signed-arith-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("isize signed arithmetic canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("i64 signed arithmetic canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("isize signed arithmetic canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected isize to be a SIGNED pointer-width integer (-42-8==-50, exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_addr_value_flow_exit_canary_runs() {
    // addr as a first-class value (param/return/local/equality) plus the
    // model's addr + u64 mixed op -- the Arena::allocate shapes.
    let canary = pass_canary(fixture_roster::RUNTIME_ADDR_VALUE_FLOW_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-addrflow-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("addr value-flow canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("addr value-flow canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("addr value-flow canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "addr value-flow canary should pass (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_addr_algebra_exit_canary_runs() {
    // The legal addr algebra: addr - count, addr - addr -> count, ordering.
    let canary = pass_canary(fixture_roster::RUNTIME_ADDR_ALGEBRA_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-addralg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("addr algebra canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("addr algebra canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("addr algebra canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "addr algebra canary should pass all legs (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_ref_param_method_dispatch_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_REF_PARAM_METHOD_DISPATCH_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-ref-param-method-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("ref-param method dispatch canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("ref-param method dispatch canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("ref-param method dispatch canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a method call on a `&mut Data` reference param to resolve to the data's \
         attached machine (Circle::code() == 99 -> exit 70); an unresolved call returns 0 \
         (exit 71). got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_typed_two_method_receivers_exit_canary_runs() {
    // Two data types implement a SAME-NAMED method (`Circle::code` == 9,
    // `Square::code` == 4), each called through a typed `&mut` reference param.
    // The inline value fold matched callee leafs by state NAME, so the
    // lexically-first impl won at every call site (both calls 9 -> n == 99).
    // Receiver-type discrimination keeps them apart: n == 9*10+4 == 94 -> 70.
    let canary = pass_canary(fixture_roster::RUNTIME_TYPED_TWO_METHOD_RECEIVERS_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-typed-two-method-receivers-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("typed two-method receivers canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("typed two-method receivers canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("typed two-method receivers canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected same-named methods on two data types to dispatch by the \
         receiver's static type (9*10+4 == 94 -> exit 70); the name-keyed fold \
         picked the first impl for both calls (99 -> exit 71). got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dyn_single_impl_dispatch_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DYN_SINGLE_IMPL_DISPATCH_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-dyn-single-impl-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dyn single-impl dispatch canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dyn single-impl dispatch canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dyn single-impl dispatch canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `&mut dyn Shape` to devirtualize to the single impl Circle and dispatch \
         Circle::code() == 99 -> exit 70; pre-devirtualization dyn dispatch returned 0 \
         (exit 71). got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_local_named_dyn_devirtualized_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_NAMED_DYN_DEVIRTUALIZED_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-local-named-dyn-devirtualized-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("a local named dynamic coercion should devirtualize through its exact row");

    let executable = compilation
        .checked_native_executable_path()
        .expect("local named dynamic canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("local named dynamic canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the exact Primary row to call through the original self.item place; got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_local_named_dyn_pass_through_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_NAMED_DYN_PASS_THROUGH_EXIT);
    for target in ["linux_x86_64", "linux_arm64"] {
        let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
            &canary.join("main.omg"),
            Some(target),
        ))
        .expect("forwarded fixture should reach checked provider custody");
        let permission_policy = native_realization::terminal_authority_permission_policy_with_rows(
            checked
                .selected_provider_plans()
                .plans()
                .iter()
                .flat_map(|plan| {
                    plan.rows
                        .iter()
                        .filter(|&row| {
                            matches!(
                                row.binding,
                                effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
                            )
                        })
                        .map(|row| {
                            native_realization::TerminalAuthorityPermissionPolicyRow::new(
                                plan.schema.identity_digest(),
                                row.requirement_identity.clone(),
                                effects::TerminalAuthorityDisposition::from_classes([
                                    effects::TerminalAuthorityClass::ProcessTermination,
                                ]),
                            )
                        })
                })
                .collect(),
        )
        .expect("exact Console exit permission policy");
        compile_rooted_backend_canary_without_output_for_target_and_permission_policy(
            &canary,
            target,
            permission_policy,
        )
        .unwrap_or_else(|diagnostics| {
            panic!(
                "{target} should link the forwarded descriptor's exact private realization:\n{}",
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
    }
    #[cfg(target_os = "linux")]
    {
        let build_dir = std::env::temp_dir().join(format!(
            "omega-runtime-local-named-dyn-pass-through-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);

        let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
            .expect("a forwarded named dynamic value should emit its exact private table function");
        assert_native_exit_code(
            &compilation,
            70,
            "forwarded named dynamic descriptor canary",
            "the indirect slot must execute against the selected Item instance, not the same-type decoy",
        );

        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn token_bound_machine_operand_selection_exit_canary_runs() {
    // `left + right` over `&Wrapped` operands selects `machine + Wrapped::add`
    // and is supplied by that declaration's own checked body through the
    // ordinary call edge: no operator evaluator, no satisfier search. Both
    // the oracle and the native host must see the token route and the named
    // route compute the same 260 (exit 70; 71 = token route missed, 72 =
    // named route missed).
    //
    // The native leg was held back on the claim that borrowed local data
    // arguments to a free machine stop at the macOS hosted receiver bridge.
    // That is no longer so, and the token supply was never the reason: the
    // control -- this program with a plain `machine Wrapped::add` and the
    // token route replaced by a second named call -- reaches the same
    // native exit 70.
    let canary = pass_canary("expressions/token_bound_machine_operand_selection");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("token-bound machine operand selection should reach checked trees");
    let interpreted = interpret(&checked, b"");
    assert_eq!(
        interpreted.error, None,
        "reference execution should run the declaration body for the token call"
    );
    assert_eq!(
        interpreted.exit_code, 70,
        "the token call and the named call should both return the wrapped sum 260"
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-token-bound-operand-selection-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("token-bound machine operand selection should compile for the native host");
    let executable = compilation
        .checked_native_executable_path()
        .expect("the token-bound canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("the token-bound canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the token route and the named route to agree natively \
         (exit 70), got {:?} (71 = token route missed, 72 = named route \
         missed)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn declared_operator_match_result_canary_interprets_both_arms() {
    // OPERATOR-MACHINE-SUPPLY acceptance: the selected true arm runs
    // `machine + Wrapped::add`'s own body and yields the wrapped sum 260u64;
    // the false arm yields 1 without invoking the operator; the named call
    // reaches the same body. Checked-only by fixture shape: the three
    // entries are each their own acceptance, so there is no `Main::main`
    // and no `build.omg`, and native production requires one exact selected
    // program entry. The operand-selection canary carries the same operator
    // declaration natively, so no lowering wall is involved.
    let canary = pass_canary("expressions/declared_operator_match_result");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("declared operator match result should reach checked trees");
    for (entry, expected) in [("select_true", 260), ("select_false", 1), ("by_name", 260)] {
        let outcome = checked_interpreter::interpret_entry(
            &checked,
            BuildMachineEntry::Name(entry),
            &[],
            checked_interpreter::InterpretOptions::default(),
        );
        assert_eq!(outcome.error, None, "{entry}");
        assert_eq!(outcome.exit_code, expected, "{entry}");
    }
}
