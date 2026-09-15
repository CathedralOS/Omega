use super::fixture_roster;
use crate::{Command, compile_rooted_canary_for_native_host, fs, pass_canary};

#[test]
fn runtime_subslice_len_exit_canary_runs() {
    // A `&[u8]` bound to a literal fixed-array subslice (`self.source[0..2]`)
    // and used only for `.len` is inlined to `(self.source[0..2]).len`; the
    // length must FOLD to the window width `b - a` (2), not fall through to a
    // place read with no descriptor slot. Exits 70 when `s.len == 2`.
    let canary = pass_canary(fixture_roster::RUNTIME_SUBSLICE_LEN_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-subslice-len-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("subslice len canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("subslice len canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("subslice len canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `(arr[0..2]).len` to fold to 2 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_index_read_dispatch_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_INDEX_READ_DISPATCH_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-read-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime dispatch slice index read canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime dispatch slice index read canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("runtime dispatch slice index read canary should run");

    assert_eq!(
        output.status.code(),
        Some(43),
        "expected runtime dispatch slice index read canary to preserve dynamic slice reads and exit 43, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_index_copy_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_INDEX_COPY_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-slice-copy-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime slice index copy canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("slice index copy canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime slice index copy canary should run");

    assert_eq!(
        output.status.code(),
        Some(51),
        "expected runtime slice index copy canary to preserve dynamic element copies and exit 51, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_index_copy_dispatch_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_INDEX_COPY_DISPATCH_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-copy-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime dispatch slice index copy canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dispatch slice index copy canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime dispatch slice index copy canary should run");

    assert_eq!(
        output.status.code(),
        Some(61),
        "expected runtime dispatch slice index copy canary to preserve dynamic element copies and exit 61, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_frame_array_slice_parameter_alias_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_FRAME_ARRAY_SLICE_PARAMETER_ALIAS_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-frame-array-slice-parameter-alias-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime frame array slice parameter alias canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("frame-array slice parameter alias canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime frame array slice parameter alias canary should run");

    assert_eq!(
        output.status.code(),
        Some(72),
        "expected a slice made from a by-value frame parameter's inline array to \
         preserve its backing storage across the transition into a slice-parameter \
         state, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_len_transition_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_LEN_TRANSITION_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-len-transition-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime slice len transition canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime slice len transition canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("runtime slice len transition canary should run");

    assert_eq!(
        output.status.code(),
        Some(101),
        "expected runtime slice len transition canary to preserve slice descriptors across transitions and exit 101, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_param_bounded_range_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SUBSLICE_PARAM_BOUNDED_RANGE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-param-bounded-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime subslice param bounded range canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("bounded parameter subslice canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime subslice param bounded range canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a bounded literal subslice of a runtime slice param to materialize length 3 and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_param_end_only_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SUBSLICE_PARAM_END_ONLY_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-param-end-only-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime subslice param end-only canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("end-only parameter subslice canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime subslice param end-only canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected an end-only subslice of a runtime slice param to materialize length 2 and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_param_local_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SUBSLICE_PARAM_LOCAL_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-param-local-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime subslice param local canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("local parameter subslice canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime subslice param local canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a subslice of a slice param assigned to a local to shrink the descriptor and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_runtime_start_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SUBSLICE_RUNTIME_START_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-runtime-start-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime subslice runtime start canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime-start subslice canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime subslice runtime start canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a runtime-start subslice (sub[start..]) to offset the descriptor pointer and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_runtime_end_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SUBSLICE_RUNTIME_END_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-runtime-end-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime subslice runtime end canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime-end subslice canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime subslice runtime end canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a runtime-end subslice (sub[..end]) to take the runtime length and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_nested_of_param_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SUBSLICE_NESTED_OF_PARAM_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-nested-param-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime nested subslice of param canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("nested parameter subslice canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime nested subslice of param canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a nested subslice (sub[1..][1..]) over a runtime slice param to compose biases and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_runtime_start_over_local_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SUBSLICE_RUNTIME_START_OVER_LOCAL_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-start-over-local-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime subslice runtime start over local canary should compile from its authored root",
    );

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime-start-over-local subslice canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime subslice runtime start over local canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a runtime-start subslice over a subslice local (tail[start..]) to compose and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_param_inclusive_end_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SUBSLICE_PARAM_INCLUSIVE_END_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-inclusive-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime subslice param inclusive end canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("inclusive-end parameter subslice canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime subslice param inclusive end canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected an inclusive-end subslice (sub[1..=3]) over a runtime slice param to fold to end + 1 and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_range_len_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SUBSLICE_RANGE_LEN_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-subslice-len-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime subslice range len canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("subslice range-length canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime subslice range len canary should run");

    assert_eq!(
        output.status.code(),
        Some(203),
        "expected runtime subslice range len canary to materialize the shortened descriptor length and exit 203, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_bounded_range_len_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SUBSLICE_BOUNDED_RANGE_LEN_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-bounded-len-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime bounded subslice range len canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("bounded subslice range-length canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime bounded subslice range len canary should run");

    assert_eq!(
        output.status.code(),
        Some(215),
        "expected runtime bounded subslice range len canary to materialize the two-sided descriptor length and exit 215, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_range_pointer_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SUBSLICE_RANGE_POINTER_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-pointer-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime subslice range pointer canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("subslice range-pointer canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime subslice range pointer canary should run");

    assert_eq!(
        output.status.code(),
        Some(205),
        "expected runtime subslice range pointer canary to offset the descriptor pointer and exit 205, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}
