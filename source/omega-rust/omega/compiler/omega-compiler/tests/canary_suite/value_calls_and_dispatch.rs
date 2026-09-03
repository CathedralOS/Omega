use super::*;

fn assert_native_exit_code(
    report: &CompileReport,
    expected: i32,
    fixture: &str,
    expectation: &str,
) {
    let executable = report
        .checked_native_executable_path()
        .unwrap_or_else(|| panic!("{fixture} lost its exact executable publication receipt"));
    let output = Command::new(executable)
        .output()
        .unwrap_or_else(|error| panic!("{fixture} should run: {error}"));
    assert_eq!(
        output.status.code(),
        Some(expected),
        "{expectation}; expected exit {expected}, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn runtime_indexed_copy_aggregate_handoff_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_indexed_copy_aggregate_handoff_exit");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
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
    let canary = pass_canary("calls/runtime_mutable_call_before_transition_args_exit");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
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
    let canary = pass_canary("calls/runtime_referenced_local_outlives_sibling_guard_call_exit");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
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
    let canary = pass_canary("borrow/runtime_view_linked_input_unrelated_ref_write_exit");
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
    let canary = pass_canary("calls/runtime_value_call_single_execution_exit");
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
    let canary = pass_canary("calls/runtime_explicit_discard_executes_exit");
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
    let canary = pass_canary("calls/runtime_transition_subject_call_single_evaluation_exit");
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
    let canary = pass_canary("control_flow/runtime_nonplace_record_pattern_single_evaluation_exit");
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
    let canary = pass_canary("control_flow/runtime_effectful_subject_single_evaluation_exit");
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
    let canary = pass_canary("control_flow/runtime_statement_call_single_execution_exit");
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
    let canary = pass_canary("calls/runtime_assignment_call_post_mutation_value_exit");
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
    let canary = pass_canary("calls/runtime_value_call_return_types_exit");
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
    let canary = pass_canary("calls/runtime_value_call_struct_result_to_target_exit");
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
    let canary = pass_canary("calls/runtime_value_call_self_field_enum_match_exit");
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
    let canary = pass_canary("calls/runtime_value_call_struct_literal_arms_exit");
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
    let canary = pass_canary("calls/runtime_contained_machine_exit");
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
    let canary = pass_canary("calls/runtime_call_result_after_splice_mutation_exit");
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
    let canary = pass_canary("calls/runtime_called_machine_loop_search_exit");
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
    let canary = pass_canary("calls/runtime_trailing_local_return_exit");
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
    let canary = pass_canary("calls/runtime_looping_value_return_exit");
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
    let canary = pass_canary("calls/runtime_looping_cast_return_exit");
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
    let canary = pass_canary("calls/runtime_value_call_slice_len_guard_exit");
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
    let canary = pass_canary("host/runtime_sleep_exit");
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
    // checks the exact stdout ("ABC\n"); this run-test just confirms it exits 70.
    let canary = pass_canary("host/runtime_write_no_newline_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-write-no-newline-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("write-no-newline canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "write-without-newline canary",
        "write followed by write_line should reach the expected exit",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_exit_code_exit_canary_runs() {
    // `exit_process(self.v)` with a RUNTIME (non-constant) i32 must exit with the
    // computed value. Regression guard for the documented footgun where a runtime
    // exit-code operand was ignored and the process silently exited 0. The canary
    // computes 5 + 65 = 70 and exits with `self.v`.
    let canary = pass_canary("calls/runtime_exit_code_exit");
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
            hosted_main_program_entry_build(target),
        )
        .expect("write runtime exit-code cross-target manifest");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("expressions/borrow_carrying_data_field_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("types/runtime_u8_field_arith_exit");
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
    let canary = pass_canary("types/runtime_i8_signed_arith_exit");
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
    let canary = pass_canary("types/runtime_i16_signed_arith_exit");
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
    let canary = pass_canary("types/runtime_u16_field_arith_exit");
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
    let canary = pass_canary("types/runtime_addr_field_exit");
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
    let canary = pass_canary("types/runtime_i64_signed_arith_exit");
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
    let canary = pass_canary("types/runtime_addr_value_flow_exit");
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
    let canary = pass_canary("types/runtime_addr_algebra_exit");
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
    let canary = pass_canary("traits/runtime_ref_param_method_dispatch_exit");
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
    let canary = pass_canary("traits/runtime_typed_two_method_receivers_exit");
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
    let canary = pass_canary("traits/runtime_dyn_single_impl_dispatch_exit");
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
    let canary = pass_canary("traits/runtime_local_named_dyn_devirtualized_exit");
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
    let canary = pass_canary("traits/runtime_local_named_dyn_pass_through_exit");
    for target in ["linux_x86_64", "linux_arm64"] {
        let checked = compile_to_checked(&canary.join("main.omg"), Some(target))
            .expect("forwarded fixture should reach checked provider custody");
        let permission_policy =
            omega_terminal_psi_to_native_artifact::terminal_authority_permission_policy_with_rows(
                checked
                    .selected_provider_plans()
                    .plans()
                    .iter()
                    .flat_map(|plan| {
                        plan.rows.iter().filter(|&row| matches!(
                                row.binding,
                                omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic {
                                    ..
                                }
                            )).map(|row| omega_terminal_psi_to_native_artifact::TerminalAuthorityPermissionPolicyRow::new(
                                    plan.schema.identity_digest(),
                                    row.requirement_identity.clone(),
                                    omega_effects::TerminalAuthorityDisposition::from_classes([
                                        omega_effects::TerminalAuthorityClass::ProcessTermination,
                                    ]),
                                ))
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

fn assert_forwarded_dynamic_result_canary(
    fixture: &str,
    expected_transfer_count: usize,
    description: &str,
    native_expectation: &str,
) {
    let _ = native_expectation;
    let canary = pass_canary(fixture);
    for target in ["linux_x86_64", "linux_arm64"] {
        let checked = compile_to_checked(&canary.join("main.omg"), Some(target)).unwrap_or_else(
            |diagnostics| {
                panic!(
                    "{description} should reach checked descriptor custody for {target}:\n{}",
                    diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.message.as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            },
        );
        assert_eq!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .dynamic_dispatch
                .transfers
                .len(),
            expected_transfer_count
        );
        assert_eq!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .dynamic_dispatch
                .rebound_scalar_calls
                .len(),
            1
        );
        let permission_policy =
            omega_terminal_psi_to_native_artifact::terminal_authority_permission_policy_with_rows(
                checked
                    .selected_provider_plans()
                    .plans()
                    .iter()
                    .flat_map(|plan| {
                        plan.rows.iter().filter(|&row| matches!(
                                row.binding,
                                omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic {
                                    ..
                                }
                            )).map(|row| omega_terminal_psi_to_native_artifact::TerminalAuthorityPermissionPolicyRow::new(
                                    plan.schema.identity_digest(),
                                    row.requirement_identity.clone(),
                                    omega_effects::TerminalAuthorityDisposition::from_classes([
                                        omega_effects::TerminalAuthorityClass::ProcessTermination,
                                    ]),
                                ))
                    })
                    .collect(),
            )
            .expect("exact Console exit permission policy");
        let report = compile_rooted_backend_canary_without_output_for_target_and_permission_policy(
            &canary,
            target,
            permission_policy,
        )
        .unwrap_or_else(|diagnostics| {
            panic!(
                "{target} should retain {description} through its exact adapter:\n{}",
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
        if fixture == "traits/runtime_local_named_dyn_boolean_pass_through_exit" {
            assert_boolean_forwarded_native_custody(&report, target);
        } else if fixture == "traits/runtime_local_named_dyn_mutable_pass_through_exit" {
            let native = report
                .retained_native_artifact()
                .expect("fixed-integer mutation keeps native custody");
            let object = native.object();
            let stores = object
                .functions()
                .iter()
                .flat_map(|function| &function.scalar_structural_scalar_field_stores)
                .collect::<Vec<_>>();
            let [integer_store, boolean_store, short_store] = stores.as_slice() else {
                panic!("{target} should retain three ordered realization stores")
            };
            assert!(integer_store.path.is_empty());
            assert_eq!(integer_store.field_byte_offset, 0);
            assert!(matches!(
                integer_store.immediate,
                omega_target_operations::TargetScalarImmediate::Integer {
                    scalar_type,
                    value: psi_core::IntegerValue::Unsigned(513),
                } if scalar_type == psi_core::IntegerType::new(
                    psi_core::IntegerSign::Unsigned,
                    64,
                ).expect("u64 type")
            ));
            assert!(boolean_store.path.is_empty());
            assert_eq!(boolean_store.field_byte_offset, 8);
            assert!(matches!(
                boolean_store.immediate,
                omega_target_operations::TargetScalarImmediate::Boolean(true)
            ));
            assert!(short_store.path.is_empty());
            assert_eq!(short_store.field_byte_offset, 10);
            assert!(matches!(
                short_store.immediate,
                omega_target_operations::TargetScalarImmediate::Integer {
                    scalar_type,
                    value: psi_core::IntegerValue::Unsigned(257),
                } if scalar_type == psi_core::IntegerType::new(
                    psi_core::IntegerSign::Unsigned,
                    16,
                ).expect("u16 type")
            ));
            assert!(integer_store.operation_ordinal < boolean_store.operation_ordinal);
            assert!(boolean_store.operation_ordinal < short_store.operation_ordinal);
            assert_eq!(
                boolean_store.code_offset,
                integer_store.code_offset + integer_store.byte_count
            );
            assert_eq!(
                short_store.code_offset,
                boolean_store.code_offset + boolean_store.byte_count
            );
        } else if fixture
            == "traits/runtime_local_named_dyn_mutable_projected_boolean_pass_through_exit"
        {
            let object = report
                .retained_native_artifact()
                .expect("nested mutation keeps native custody")
                .object();
            let stores = object
                .functions()
                .iter()
                .flat_map(|function| &function.scalar_structural_scalar_field_stores)
                .collect::<Vec<_>>();
            let [store] = stores.as_slice() else {
                panic!("{target} should retain one nested realization store")
            };
            assert_eq!(
                store.path,
                [
                    psi_terminal::StructuralPathSegment::Field("envelope".into()),
                    psi_terminal::StructuralPathSegment::Field("flags".into()),
                ]
            );
            assert_eq!(store.field_byte_offset, 8);
        } else if fixture == "traits/runtime_local_named_dyn_multi_hop_pass_through_exit" {
            let object = report
                .retained_native_artifact()
                .expect("multi-hop forwarding keeps native custody")
                .object();
            assert_eq!(
                object
                    .functions()
                    .iter()
                    .flat_map(|function| &function.forwarded_dynamic_descriptor_calls)
                    .count(),
                1,
                "{target} must retain the selection-sourced descriptor call"
            );
            assert_eq!(
                object
                    .functions()
                    .iter()
                    .flat_map(|function| &function.forwarded_dynamic_parameter_calls)
                    .count(),
                1,
                "{target} must retain the intermediate parameter-forwarding direct call"
            );
            assert_eq!(
                object
                    .functions()
                    .iter()
                    .flat_map(|function| &function.dynamic_parameter_calls)
                    .count(),
                1,
                "{target} must retain the final parameter-sourced indirect call"
            );
        }
    }
    #[cfg(target_os = "linux")]
    {
        let build_dir = std::env::temp_dir().join(format!(
            "omega-{}-{}",
            fixture.replace(['/', '_'], "-"),
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);

        let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "{description} should emit its exact private table function:\n{}",
                    diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.message.as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            });
        assert_native_exit_code(&compilation, 70, description, native_expectation);

        let _ = fs::remove_dir_all(&build_dir);
    }
}

fn assert_boolean_forwarded_native_custody(report: &CompileReport, target: &str) {
    let native = report
        .retained_native_artifact()
        .expect("Boolean forwarded result keeps native custody");
    let object = native.object();
    let forwarded = object
        .functions()
        .iter()
        .filter_map(|function| {
            let [call] = function.forwarded_dynamic_descriptor_calls.as_slice() else {
                return None;
            };
            Some((function, call))
        })
        .collect::<Vec<_>>();
    let [(caller, call)] = forwarded.as_slice() else {
        panic!("{target} should retain one forwarded Boolean result call")
    };
    let semantic_result = call.semantic_result.expect("Boolean semantic result");
    let result = call.result.as_ref().expect("Boolean physical result");
    assert_eq!(semantic_result.scalar_type, psi_core::ScalarType::Boolean);
    assert_eq!(result.home.scalar_type, psi_core::ScalarType::Boolean);
    assert_eq!(
        result.home.shape,
        omega_calling_conventions::ValueShape::integer(1, 1)
    );
    assert!(object.functions().iter().any(|function| {
        function
            .mixed_structural_scalar_abi
            .as_ref()
            .is_some_and(|abi| abi.result.scalar_type == psi_core::ScalarType::Boolean)
    }));
    let branch_offset = call.code_offset + call.byte_count;
    let bytes = caller.bytes(object);
    match target {
        "linux_x86_64" => assert!(
            bytes[branch_offset..]
                .windows(3)
                .next()
                .is_some_and(|prefix| prefix == [0x40, 0x0f, 0xb6])
                && bytes[branch_offset..branch_offset + 20]
                    .windows(4)
                    .any(|window| window == [0x84, 0xc0, 0x0f, 0x84]),
            "x86-64 must load the one-byte home, test AL, and branch false on zero"
        ),
        "linux_arm64" => {
            let compare = u32::from_le_bytes(
                bytes[branch_offset + 4..branch_offset + 8]
                    .try_into()
                    .expect("AArch64 compare word"),
            );
            let branch = u32::from_le_bytes(
                bytes[branch_offset + 8..branch_offset + 12]
                    .try_into()
                    .expect("AArch64 branch word"),
            );
            assert_eq!(compare, 0x7100_013f, "AArch64 must compare w9 with zero");
            assert_eq!(
                branch & 0xff00_001f,
                0x5400_0000,
                "AArch64 must branch false with b.eq"
            );
        }
        _ => unreachable!("bounded target roster"),
    }
}

#[test]
fn runtime_local_named_dyn_mutable_pass_through_exit_canary_runs() {
    assert_forwarded_dynamic_result_canary(
        "traits/runtime_local_named_dyn_mutable_pass_through_exit",
        1,
        "mutable forwarded named dynamic descriptor canary",
        "the indirect slot must preserve exclusive access and select the rebound Item instance",
    );
}

#[test]
fn runtime_local_named_dyn_boolean_pass_through_exit_canary_runs() {
    assert_forwarded_dynamic_result_canary(
        "traits/runtime_local_named_dyn_boolean_pass_through_exit",
        1,
        "forwarded named dynamic Boolean result canary",
        "the indirect slot must preserve the selected true result through its durable Boolean home",
    );
}

#[test]
fn runtime_local_named_dyn_mutable_boolean_pass_through_exit_canary_runs() {
    assert_forwarded_dynamic_result_canary(
        "traits/runtime_local_named_dyn_mutable_boolean_pass_through_exit",
        1,
        "mutable forwarded named dynamic Boolean descriptor canary",
        "the indirect slot must store true into the rebound Item before returning its independent code field",
    );
}

#[test]
fn runtime_local_named_dyn_mutable_projected_boolean_pass_through_exit_canary_runs() {
    assert_forwarded_dynamic_result_canary(
        "traits/runtime_local_named_dyn_mutable_projected_boolean_pass_through_exit",
        1,
        "projected mutable forwarded named dynamic Boolean descriptor canary",
        "the indirect slot must store true through the nested Envelope/Flags path before returning the selected Item code",
    );
}

#[test]
fn runtime_local_named_dyn_multi_hop_pass_through_exit_canary_runs() {
    assert_forwarded_dynamic_result_canary(
        "traits/runtime_local_named_dyn_multi_hop_pass_through_exit",
        2,
        "multi-hop forwarded named dynamic descriptor canary",
        "both unchanged descriptor handoffs must reach the selected Item and return 99 to the caller's exit diamond",
    );
}

#[test]
fn runtime_local_named_dyn_unit_multi_hop_return_canary_runs() {
    let canary = pass_canary("traits/runtime_local_named_dyn_unit_multi_hop_return");
    for target in ["linux_x86_64", "linux_arm64"] {
        let report = compile_rooted_backend_canary_without_output_for_target(&canary, target)
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "Unit descriptor chain should reach native custody for {target}:\n{}",
                    diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.message.as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            });
        let object = report
            .retained_native_artifact()
            .expect("Unit forwarding keeps native custody")
            .object();
        assert_eq!(
            object
                .functions()
                .iter()
                .flat_map(|function| &function.forwarded_dynamic_descriptor_calls)
                .count(),
            1,
            "{target} must retain the selection-sourced Unit descriptor call"
        );
        let forwarded = object
            .functions()
            .iter()
            .flat_map(|function| &function.forwarded_dynamic_parameter_calls)
            .collect::<Vec<_>>();
        let [call] = forwarded.as_slice() else {
            panic!("{target} should retain one Unit parameter-forwarding call")
        };
        assert!(call.source_value.is_none());
        assert!(call.scalar_type.is_none());
        assert!(matches!(
            call.call_stack,
            omega_machine_code::ForwardedDynamicParameterCallStackEvidence::Unit(_)
        ));
        assert_eq!(
            object
                .functions()
                .iter()
                .flat_map(|function| &function.dynamic_parameter_calls)
                .count(),
            1,
            "{target} must retain the final Unit parameter-sourced indirect call"
        );
    }
    #[cfg(target_os = "linux")]
    {
        let build_dir = std::env::temp_dir().join(format!(
            "omega-runtime-local-named-dyn-unit-multi-hop-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);
        let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
            .expect("Unit descriptor chain should emit a native image");
        assert_native_exit_code(
            &compilation,
            0,
            "Unit descriptor multi-hop canary",
            "the result-less descriptor must cross both helpers and return normally",
        );
        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn runtime_local_named_dyn_rebound_direct_exit_canary_runs() {
    let canary = pass_canary("traits/runtime_local_named_dyn_rebound_direct_exit");
    for target in ["linux_x86_64", "linux_arm64"] {
        let checked = compile_to_checked(&canary.join("main.omg"), Some(target))
            .expect("rebound fixture should reach checked provider custody");
        let permission_policy =
            omega_terminal_psi_to_native_artifact::terminal_authority_permission_policy_with_rows(
                checked
                    .selected_provider_plans()
                    .plans()
                    .iter()
                    .flat_map(|plan| {
                        plan.rows.iter().filter(|&row| matches!(
                                row.binding,
                                omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic {
                                    ..
                                }
                            )).map(|row| omega_terminal_psi_to_native_artifact::TerminalAuthorityPermissionPolicyRow::new(
                                    plan.schema.identity_digest(),
                                    row.requirement_identity.clone(),
                                    omega_effects::TerminalAuthorityDisposition::from_classes([
                                        omega_effects::TerminalAuthorityClass::ProcessTermination,
                                    ]),
                                ))
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
                "{target} should lower the rebound dynamic call and its exact exit diamond:\n{}",
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
    }

    // The closed compiler-builtin catalog currently realizes exit_process on
    // Linux. Keep the machine-code construction checks above cross-target and
    // execute the artifact on Linux hosts where that exact settlement exists.
    #[cfg(target_os = "linux")]
    {
        let build_dir = std::env::temp_dir().join(format!(
            "omega-runtime-local-named-dyn-rebound-direct-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);

        let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
            .expect("a rebound named dynamic value should emit its exact private table function");
        assert_native_exit_code(
            &compilation,
            70,
            "rebound named dynamic descriptor canary",
            "the indirect slot must execute against the rebound Item instance, not the original decoy",
        );

        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn runtime_local_named_dyn_stored_return_canary_runs() {
    let canary = pass_canary("traits/runtime_local_named_dyn_stored_return");
    for target in ["linux_x86_64", "linux_arm64"] {
        let report = compile_rooted_backend_canary_without_output_for_target(&canary, target)
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "{target} should lower the stored descriptor call:\n{}",
                    diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.message.as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            });
        assert_eq!(
            report
                .retained_native_artifact()
                .expect("stored descriptor keeps native custody")
                .object()
                .functions()
                .iter()
                .flat_map(|function| &function.stored_dynamic_calls)
                .count(),
            1,
            "{target} must retain the stored descriptor call"
        );
    }

    #[cfg(target_os = "linux")]
    {
        let build_dir = std::env::temp_dir().join(format!(
            "omega-runtime-local-named-dyn-stored-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);
        let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
            .expect("a stored named dynamic value should emit a native image");
        assert_native_exit_code(
            &compilation,
            0,
            "stored named dynamic descriptor canary",
            "the later field reload and indirect call must return normally",
        );
        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn runtime_local_named_dyn_stored_exit_canary_runs() {
    let canary = pass_canary("traits/runtime_local_named_dyn_stored_exit");
    for target in ["linux_x86_64", "linux_arm64"] {
        let report =
            compile_rooted_backend_canary_without_output_for_target_with_fixture_permissions(
                &canary, target,
            )
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "{target} should lower stored descriptor result control:\n{}",
                    diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.message.as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            });
        assert_eq!(
            report
                .retained_native_artifact()
                .expect("stored descriptor control keeps native custody")
                .object()
                .functions()
                .iter()
                .flat_map(|function| &function.stored_dynamic_calls)
                .count(),
            1,
            "{target} must retain the stored descriptor result call"
        );
    }

    #[cfg(target_os = "linux")]
    {
        let build_dir = std::env::temp_dir().join(format!(
            "omega-runtime-local-named-dyn-stored-exit-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);
        let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
            .expect("stored named dynamic result control should emit a native image");
        assert_native_exit_code(
            &compilation,
            70,
            "stored named dynamic descriptor result canary",
            "the reloaded descriptor result must drive the good exit arm",
        );
        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn runtime_dyn_two_impl_dispatch_exit_canary_runs() {
    // TWO data types satisfy Shape, so the `&mut dyn Shape` receiver cannot
    // devirtualize; the call is monomorphized over the trait's closed world and
    // each call site's receiver type picks the impl: Circle::code() == 9 then
    // Square::code() == 4 -> n == 94 -> exit 70. Mirrors the interpreter
    // coverage test dyn_two_impl_dispatch_selects_impl_by_runtime_type.
    let canary = pass_canary("traits/runtime_dyn_two_impl_dispatch_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-dyn-two-impl-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dyn two-impl dispatch canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dyn two-impl dispatch canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dyn two-impl dispatch canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `&mut dyn Shape` with TWO impls to dispatch by the call site's \
         receiver type (Circle 9, Square 4 -> n == 94 -> exit 70); an unresolved \
         dyn call returns 0 for both (exit 71). got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dyn_two_impl_dispatch_swapped_exit_canary_runs() {
    // Same two impls, call order swapped: Square (4) first, then Circle (9)
    // -> n == 49 -> exit 70. A dispatcher that always picks the lexically-first
    // impl cannot pass both this and the unswapped canary (it scores 99 twice).
    let canary = pass_canary("traits/runtime_dyn_two_impl_dispatch_swapped_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-dyn-two-impl-dispatch-swapped-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dyn two-impl swapped dispatch canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dyn two-impl swapped dispatch canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dyn two-impl swapped dispatch canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the swapped call order to dispatch Square (4) then Circle (9) \
         -> n == 49 -> exit 70. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_alias_write_through_guarded_transition_exit_canary_runs() {
    // A `&mut` param forwarded through a GUARDED transition into a sub-state that
    // writes through it must reach the caller's object. When the callee inlines as a
    // branching leaf, by-value args bind as `mut <literal>` (e.g. `mut 2`), so the
    // leaf guard `key < 4` carried a `Mutable(Integer)` operand the value resolvers
    // didn't see through -- the arm (guard + its alias write) was dropped entirely.
    let canary = pass_canary("calls/runtime_alias_write_through_guarded_transition_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-alias-write-guarded-transition-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("alias-write-through-guarded-transition canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "alias-write-through-guarded-transition canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("alias-write-through-guarded-transition canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a &mut alias written in a sub-state reached via a guarded transition \
         to reach the caller (exit 70), got {:?} (71 = the guarded arm's alias write was \
         dropped)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_reference_param_forwarded_through_loop_exit_canary_runs() {
    // Forwarding a `&mut` param to another `&mut` param through a (self-looping)
    // dispatch transition must copy the POINTER VALUE, not the pointee. The materializer
    // dereferenced whenever the referent size equalled the target slot size; for a
    // pointer-sized referent it wrote room data into the pointer slot, so the next write
    // through it faulted. The deref branch now fires only for VALUE targets.
    let canary = pass_canary("calls/runtime_reference_param_forwarded_through_loop_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-reference-param-forwarded-loop-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("reference-param-forwarded-through-loop canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "reference-param-forwarded-through-loop canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("reference-param-forwarded-through-loop canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a &mut param forwarded to another &mut param through a loop to copy the \
         pointer value (exit 70), got {:?} (139/segfault = pointee written into pointer slot)\
         \nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_value_call_through_alias_in_dispatch_exit_canary_runs() {
    // A value-returning inline-branching call written through a `&mut` alias inside a
    // DISPATCHED callee must yield the matched arm's value. The guard's forward-skip
    // distance must cover the per-arm pointee copy; `is_guarded_effect` was missing
    // RuntimeStorageCopyToRuntimePointee / RuntimePointee{Integer,Binary}Write, so a
    // skipped arm ran the pointee copy unconditionally and stranded the matched arm.
    let canary = pass_canary("calls/runtime_value_call_through_alias_in_dispatch_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-value-call-alias-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("value-call-through-alias-in-dispatch canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("value-call-through-alias-in-dispatch canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("value-call-through-alias-in-dispatch canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a branching-call result written through a &mut alias in a dispatched callee \
         to yield the matched arm (exit 70), got {:?} (71 = a skipped arm's pointee copy ran \
         unconditionally and stranded the match)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_value_call_in_substate_exit_canary_runs() {
    // Two-level nested call from a sub-state: hall1 -> carve (statement) -> room_mut
    // (in `let x = self.f()` position). carve was misclassified as a leaf (leaf check
    // only scanned `OperationKind::Call` ops, missing the call in a `let` initializer),
    // so its nested room_mut was dropped -> null `&mut Room` -> fault. The classifier
    // now treats a state that sources any non-host call as non-leaf (InlineExpansion).
    let canary = pass_canary("calls/runtime_nested_value_call_in_substate_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-nested-value-call-substate-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested-value-call-in-substate canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("nested-value-call-in-substate canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested-value-call-in-substate canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a 2-level nested call (sub-state -> helper -> value-position call) to be \
         expanded (exit 70), got {:?} (139/71 = the helper was treated as a leaf and its \
         nested call dropped)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_call_in_inlined_substate_exit_canary_runs() {
    // A transition target (sub-state) that calls in `let x = self.f()` position must
    // lower as a straight-line branch so the nested call is expanded. It was
    // misclassified as a leaf (leaf check looked only for Statement-role calls), and
    // leaf expansion can't carry a nested call -> the call was dropped, leaving its
    // `&mut`/value result null -> the next use faulted. Dungeon generation shape.
    let canary = pass_canary("calls/runtime_call_in_inlined_substate_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-call-in-inlined-substate-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("call-in-inlined-substate canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("call-in-inlined-substate canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("call-in-inlined-substate canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a `let x = self.f()` call in a transition sub-state to be expanded (exit 70), \
         got {:?} (139/71 = the sub-state was treated as a leaf and the call dropped)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_alias_indexed_read_through_transition_exit_canary_runs() {
    // An inlined leaf reading `items[key].field` (constant-index element of a
    // forwarded slice) through a forwarded `&mut` alias: the inlined by-value `key`
    // binds as `mut 2`, so `items[key]` became `items[mut 2]` and the index-path
    // resolvers rejected the `Mutable`-wrapped index, dropping the copy. The `mut`
    // is now stripped on a resolved leaf index. Mirrors the dungeon find_room shape.
    let canary = pass_canary("calls/runtime_alias_indexed_read_through_transition_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-alias-indexed-read-transition-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("alias-indexed-read-through-transition canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "alias-indexed-read-through-transition canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("alias-indexed-read-through-transition canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `items[key].field` (constant index) copied through a &mut alias in a \
         guarded sub-state to resolve (exit 70), got {:?} (71 = `mut`-wrapped index rejected)\
         \nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dispatch_binary_call_argument_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_dispatch_binary_call_argument_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("binary-local call-argument canary should compile to checked trees");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-dispatch-binary-call-arg-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime dispatch binary call argument canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "runtime dispatch binary call argument canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime dispatch binary call argument canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a binary-initialized local passed as a call argument (`carve(level, tag)`) \
         from the proof-bearing guarded state to copy its slot and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A dispatched value call whose result binds to a FIELD (no frame result
// slot): the return-write resolves the caller's Assignment target to its
// machine-region place. Was a live silent-wrong (field stayed ZII).
#[test]
fn runtime_dispatch_result_field_binding_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_dispatch_result_field_binding_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-dispatch-result-field-binding-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dispatch result field-binding canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dispatch result field-binding canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dispatch result field-binding canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the dispatched call's terminal to write the caller's FIELD \
         (self.total == 5 -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A threaded mutable receiver crosses two calls, an outer guard, and a
// trailing-state mutation. Each transition guard must observe its authored
// phase rather than a flattened descendant predicate.
#[test]
fn runtime_trailing_state_mut_param_phase_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_trailing_state_mut_param_phase_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("threaded mutable receiver phase canary should compile to checked trees");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let build_dir = std::env::temp_dir().join(format!(
        "omega-trailing-mut-param-phase-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("threaded mutable receiver phase canary should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("threaded mutable receiver phase canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("threaded mutable receiver phase canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected outer and trailing guards to observe calls 2 and 3 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// The ORIGINAL same-type receiver aliasing repro, now serving:
// self.b.increment() mutates b (was: mutated a via by-type resolution).
#[test]
fn runtime_same_type_second_receiver_mutation_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_same_type_second_receiver_mutation_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-same-type-second-receiver-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("second-receiver mutation canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("second-receiver mutation canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("second-receiver mutation canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected b.value == 1 after a x1 + b x1 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// A dispatched FLOAT place terminal round-trips through the result slot
// (the type-agnostic place-copy serve; the old "floats bail" row was
// stale). Float BINARY terminals remain unserved.
#[test]
fn runtime_dispatch_float_terminal_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_dispatch_float_terminal_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-dispatch-float-terminal-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dispatch float terminal canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("dispatch float terminal canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the float place terminal to round-trip 1.5 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// std::time Duration core through NON-FIRST same-type receivers (the
// inline-route per-instance fix): sum.checked_subtract resolves against
// sum's storage, not the first Duration's.
#[test]
fn runtime_value_machine_receiver_field_postentry_exit_canary_runs() {
    let canary = pass_canary("time/runtime_value_machine_receiver_field_postentry_exit");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("receiver-field postentry canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.error, None, "should interpret cleanly");
    assert_eq!(outcome.exit_code, 70, "interp oracle should exit 70");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-receiver-field-postentry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("receiver-field postentry canary should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("receiver-field postentry canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("receiver-field postentry canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected exact Duration math through the third same-type receiver (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// A method through the SECOND same-type NESTED leaf (self.p.b.get())
// reads b's 9, not the first leaf's 5 (the inline nested-receiver fix).
#[test]
fn runtime_nested_receiver_same_type_exit_canary_runs() {
    let canary = pass_canary("references/runtime_nested_receiver_same_type_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-nested-receiver-same-type-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested same-type receiver canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested same-type receiver canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested same-type receiver canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected p.b.get() == 9 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// PER-INSTANCE receiver dispatch: a looping value machine called through
// the SECOND same-type contained receiver runs on that receiver's storage
// (21 = 3 iterations of second.count 7; by-type resolution read first's
// 100 and delivered 300).
#[test]
fn runtime_dispatch_second_receiver_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_dispatch_second_receiver_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-dispatch-second-receiver-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("second-receiver dispatch canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("second-receiver dispatch canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("second-receiver dispatch canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the SECOND receiver's storage to drive the loop (21 -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dispatch_sibling_value_calls_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_dispatch_sibling_value_calls_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-dispatch-sibling-value-calls-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("sibling dispatched value calls should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("sibling dispatched value calls should retain their executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("sibling dispatched value calls should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected both dispatched calls to retain receiver/result identity (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_inline_repeated_receiver_value_calls_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_inline_repeated_receiver_value_calls_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-inline-repeated-receiver-value-calls-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("repeated inline value calls on one receiver should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("repeated inline value calls should retain their executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("repeated inline value calls on one receiver should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected each inline call occurrence to retain its result slot (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// RECEIVER SLICE 2: the second-receiver dispatch shape with a NON-ENTRY
// caller (Holder under Main). The per-dispatch base composes through the
// parent-context chain (Main->holder@0, +second@4); also pins the
// dispatch-index table's ARENA-index (1-based) alignment -- the positional
// table read the next state's base and this is the shape that exposes it.
#[test]
fn runtime_nonentry_second_receiver_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_nonentry_second_receiver_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-nonentry-second-receiver-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("non-entry second-receiver canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("non-entry second-receiver canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("non-entry second-receiver canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the SECOND receiver's storage through a NON-entry caller (21 -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// RECEIVER SLICE 2, SELF-CALL HOP: the dispatching caller is reached
// through a machine-to-machine SELF call (Main -> holder.run() ->
// self.step() -> second.drain()). Self-call contexts inherit the parent's
// composed base, so the named-receiver hop downstream keeps composing.
#[test]
fn runtime_selfcall_chain_second_receiver_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_selfcall_chain_second_receiver_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-selfcall-chain-second-receiver-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("self-call chain second-receiver canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("self-call chain second-receiver canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("self-call chain second-receiver canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the SECOND receiver's storage through a self-call hop (21 -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// NESTED INLINE VALUE-CALL CHAIN with COLLIDING LOCAL NAMES: the outer
// leaf terminal-write must resolve the arm's arg in the CALL-TARGET scope
// first -- the case-wide name fallback previously copied Main's same-named
// (unwritten) local and delivered ZII.
#[test]
fn runtime_nested_inline_chain_result_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_nested_inline_chain_result_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-nested-inline-chain-result-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested inline chain result canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested inline chain result canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested inline chain result canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the chained inline result to deliver 7 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// RECEIVER SLICE 2, INLINE ROUTE: a non-looping value machine spliced
// through the SECOND same-type receiver from a NON-entry caller (two-hop
// splice; chain-walk recovery + call-target-first leaf-write resolution).
#[test]
fn runtime_nonentry_inline_second_receiver_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_nonentry_inline_second_receiver_exit");
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
    let canary = pass_canary("calls/runtime_nested_local_terminal_second_instance_exit");
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
    let canary = pass_canary("calls/runtime_nested_field_terminal_second_instance_exit");
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
    let canary = pass_canary("calls/runtime_multiarm_same_named_locals_exit");
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
    let canary = pass_canary("calls/runtime_multiarm_texteq_local_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("calls/runtime_pre_guard_texteq_local_guard_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("calls/runtime_pre_guard_texteq_local_arg_forward_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("calls/runtime_param_receiver_second_instance_exit");
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
    let canary = pass_canary("calls/runtime_param_forward_chain_second_receiver_exit");
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
    let canary = pass_canary("build/runtime_main_source_builder_is_ordinary_exit");
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
    let canary = pass_canary("time/runtime_saturating_time_arith_exit");
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
    let canary = pass_canary("core/runtime_natural_termination_exit");
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
    let canary = pass_canary("calls/runtime_deep_state_name_collision_exit");
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
    let canary = pass_canary("arithmetic/runtime_u64_literal_let_guard_exit");
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
    let canary = pass_canary("calls/runtime_param_receiver_single_instance_exit");
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
    let canary = pass_canary("calls/runtime_dispatch_result_alias_read_exit");
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
    let canary = pass_canary("calls/runtime_dispatch_slice_element_terminal_exit");
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
    let canary = pass_canary("calls/runtime_dispatch_result_binary_terminal_exit");
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
    let canary = pass_canary("calls/runtime_dispatch_result_multi_arm_exit");
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
    let canary = pass_canary("calls/runtime_dispatch_result_guard_subject_exit");
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
    let canary = pass_canary("calls/runtime_dispatch_result_transition_arg_exit");
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
    let canary = pass_canary("calls/runtime_dispatched_effectful_reentrant_exit");
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
    let canary = pass_canary("calls/runtime_dispatch_result_enum_case_exit");
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
    let canary = pass_canary("calls/runtime_dispatch_machine_array_slice_arg_exit");
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
    let canary = pass_canary("calls/runtime_dispatch_result_field_terminal_exit");
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
    let canary = pass_canary("calls/runtime_nested_called_machine_loop_exit");
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
    let canary = pass_canary("control_flow/runtime_state_loop_indexed_search_exit");
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
    let canary = pass_canary("calls/runtime_call_result_through_reference_field_exit");
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
    let canary = pass_canary("calls/runtime_string_call_result_through_reference_field_exit");
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
    let canary = pass_canary("calls/runtime_two_string_call_results_through_reference_fields_exit");
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
    let canary =
        pass_canary("calls/runtime_offset_string_call_results_through_reference_fields_exit");
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
    let canary = pass_canary("calls/runtime_reference_returned_slice_element_write_exit");
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
    let canary = pass_canary("calls/runtime_reference_returned_slice_element_through_param_exit");
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
    let canary = pass_canary("calls/runtime_nested_guarded_reference_returned_slice_element_exit");
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
    let canary = pass_canary("calls/runtime_mutable_local_indexed_parameter_write_exit");
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
    let canary =
        pass_canary("calls/runtime_mutable_machine_owned_local_indexed_parameter_write_exit");
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
    let canary =
        pass_canary("calls/runtime_mutable_dynamic_indexed_machine_owned_parameter_write_exit");
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
    let canary = pass_canary("storage/runtime_dispatch_local_index_binary_write_exit");
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
    let canary = pass_canary("storage/runtime_dispatch_helper_local_alias_add_exit");
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
    let canary = pass_canary("storage/runtime_slice_alias_indexed_field_write_exit");
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
    let canary = pass_canary("storage/runtime_slice_indexed_binary_rmw_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("calls/runtime_mut_ref_forward_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("storage/runtime_local_slice_forward_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("float/f32_guard_const_arith_landed_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("float/f32_arg_const_arith_landed_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
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
