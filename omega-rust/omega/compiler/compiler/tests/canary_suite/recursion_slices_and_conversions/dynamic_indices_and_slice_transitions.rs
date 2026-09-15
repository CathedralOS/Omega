use super::fixture_roster;
use crate::{Command, compile_rooted_canary_for_native_host, fs, pass_canary};

#[test]
fn runtime_local_aggregate_into_let_exit_canary_runs() {
    // A local ARRAY literal read by a subsequent `let` (`let arr = [..]; let e = arr[1]`)
    // silently yielded 0: the liveness scan never inspected LocalData (`let`) values, so
    // the read-only array was elided (no slot) and the indexed read resolved against a
    // missing slot. Fixed by keeping the slot for an array-literal local referenced in a
    // later let value (array-only -- borrow-carrying structs must stay folded).
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_AGGREGATE_INTO_LET_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-local-aggregate-into-let-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("local-aggregate-into-let canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("local aggregate into-let canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("local-aggregate-into-let canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a local array element read into a subsequent let (and used as a value) to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_field_array_element_value_operand_exit_canary_runs() {
    // A field array's indexed element as a VALUE OPERAND: passed to a value-call, and
    // read into a let then forwarded as a transition arg. Works for FIELD arrays; the
    // local-array form (`let arr = [..]; let e = arr[i]`) silently yields 0 -- a
    // machine-indexed-value-operand gap tracked separately.
    let canary = pass_canary(fixture_roster::RUNTIME_FIELD_ARRAY_ELEMENT_VALUE_OPERAND_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-field-array-value-operand-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("field-array value-operand canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("field-array value-operand canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("field-array value-operand canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a field-array element used as a value-call arg / let-then-transition-arg to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_dynamic_index_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SUBSLICE_DYNAMIC_INDEX_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-dynamic-index-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime subslice dynamic index canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("subslice dynamic-index canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime subslice dynamic index canary should run");

    assert_eq!(
        output.status.code(),
        Some(207),
        "expected runtime subslice dynamic index canary to read through the adjusted descriptor pointer and exit 207, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_bounded_dynamic_index_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SUBSLICE_BOUNDED_DYNAMIC_INDEX_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-bounded-dynamic-index-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime bounded subslice dynamic index canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("bounded subslice dynamic-index canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime bounded subslice dynamic index canary should run");

    assert_eq!(
        output.status.code(),
        Some(209),
        "expected runtime bounded subslice dynamic index canary to read through the adjusted descriptor pointer and exit 209, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_end_dynamic_index_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SUBSLICE_END_DYNAMIC_INDEX_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-end-dynamic-index-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime end subslice dynamic index canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("end subslice dynamic-index canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime end subslice dynamic index canary should run");

    assert_eq!(
        output.status.code(),
        Some(211),
        "expected runtime end subslice dynamic index canary to read through the descriptor and exit 211, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_subslice_dynamic_index_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_SUBSLICE_DYNAMIC_INDEX_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-nested-subslice-dynamic-index-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime nested subslice dynamic index canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("nested subslice dynamic-index canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime nested subslice dynamic index canary should run");

    assert_eq!(
        output.status.code(),
        Some(213),
        "expected runtime nested subslice dynamic index canary to compose descriptor windows and exit 213, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_subslice_fixed_index_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_SUBSLICE_FIXED_INDEX_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-nested-subslice-fixed-index-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime nested subslice fixed index canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("nested subslice fixed-index canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime nested subslice fixed index canary should run");

    assert_eq!(
        output.status.code(),
        Some(215),
        "expected runtime nested subslice fixed index canary to copy from the composed window and exit 215, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_fixed_index_guard_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_FIXED_INDEX_GUARD_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-fixed-index-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime slice fixed index guard canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime slice fixed index guard canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("runtime slice fixed index guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(121),
        "expected runtime slice fixed index guard canary to preserve transitioned fixed-index reads and exit 121, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_local_slice_len_comparison_value_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_SLICE_LEN_COMPARISON_VALUE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-local-slice-len-comparison-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime local slice len comparison canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime local slice len comparison canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("runtime local slice len comparison canary should run");

    assert_eq!(
        output.status.code(),
        Some(191),
        "expected runtime local slice len comparison canary to preserve slice len comparisons in local bool values and exit 191, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_index_transition_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_INDEX_TRANSITION_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-index-transition-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime slice index transition canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime slice index transition canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("runtime slice index transition canary should run");

    assert_eq!(
        output.status.code(),
        Some(111),
        "expected runtime slice index transition canary to preserve whole-element copies across transitioned slice parameters and exit 111, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_iteration_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_ITERATION_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-iteration-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime slice iteration canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime slice iteration canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("runtime slice iteration canary should run");

    assert_eq!(
        output.status.code(),
        Some(91),
        "expected runtime slice iteration canary to preserve iterative transitioned indexed reads and exit 91, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}
