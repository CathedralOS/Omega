use super::assert_native_exit_code;
use super::fixture_roster;
use crate::{compile_rooted_canary_for_native_host, fs, pass_canary};

#[test]
fn runtime_machine_frame_index_read_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MACHINE_FRAME_INDEX_READ_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-mfi-read-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("machine frame-index read canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "machine frame-index read canary",
        "the machine-owned array should read through its frame-resident index",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// MACHINE-owned array WRITE of a runtime value at a FRAME-resident index
// (machine source + frame index).

#[test]
fn runtime_machine_frame_index_write_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MACHINE_FRAME_INDEX_WRITE_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-mfi-write-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("machine frame-index write canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "machine frame-index write canary",
        "the runtime value should store through the frame-resident index",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// MACHINE-owned array WRITE with BOTH source and index frame-resident params
// -- the three-relocation case.

#[test]
fn runtime_machine_frame_index_dual_frame_write_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MACHINE_FRAME_INDEX_DUAL_FRAME_WRITE_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-mfi-dual-write-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("machine frame-index dual-frame write canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "machine dual-frame index-write canary",
        "the frame-resident source and index should drive the machine-array write",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// MACHINE-array RMW at a FRAME (param) index under a dominating guard: the
// frame-index BINARY write encoder + the whole-machine param scope in the
// index prover (k's declared range reaches the sub-state).

#[test]
fn runtime_machine_frame_index_rmw_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MACHINE_FRAME_INDEX_RMW_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-mfi-rmw-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("machine frame-index rmw canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "machine frame-index read-modify-write canary",
        "the frame-indexed machine-array read-modify-write should preserve its update",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// Machine-array reads at FRAME indices as binary operands (incl. a struct
// element field) and as a transition argument.

#[test]
fn runtime_machine_frame_index_arg_operand_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MACHINE_FRAME_INDEX_ARG_OPERAND_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-mfi-arg-operand-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("machine frame-index arg/operand canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "machine frame-index operand/argument canary",
        "frame-indexed machine-array reads should materialize as operands and arguments",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// CONST-row + RUNTIME-column 2D reads: machine-field AND frame-let consumers.

#[test]
fn runtime_nested_const_row_indexed_read_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_CONST_ROW_INDEXED_READ_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-nested-const-row-read-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("nested const-row indexed read canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "nested const-row indexed-read canary",
        "the constant row and runtime column should select the same element for both consumers",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// CONST-element + RUNTIME-leaf struct-field array WRITE, neighbor-validated.

#[test]
fn runtime_nested_const_row_struct_field_write_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_CONST_ROW_STRUCT_FIELD_WRITE_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-nested-const-row-sf-write-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("nested const-row struct-field write canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "nested const-row struct-field write canary",
        "the runtime leaf index should update only the selected nested field element",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// 3D RUNTIME-MIDDLE index (`cube[1][b][0]`): const leaf rides the suffix walk
// above the runtime level, const prefix folds into the collection resolution.

#[test]
fn runtime_nested_middle_index_3d_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_MIDDLE_INDEX_3D_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-nested-middle-3d-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("nested runtime-middle 3D canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "nested runtime-middle 3D canary",
        "the runtime middle index should preserve its constant prefix and suffix",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A range-typed LET carrying a computed index (`let m = k + 1; arr[m]`) on
// every face: read, write target, guard subject, backward offset, bare-copy.

#[test]
fn runtime_let_bound_computed_index_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_LET_BOUND_COMPUTED_INDEX_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-let-computed-index-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("let-bound computed index canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "let-bound computed-index canary",
        "the computed local index should select the right element on every access face",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// Array-of-structs element field in every binary-operand position (left/right
// operand, both-indexed, guard-dominated RMW, guard subject, indexed target).

#[test]
fn runtime_struct_field_operand_matrix_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_STRUCT_FIELD_OPERAND_MATRIX_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-sf-operand-matrix-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("struct-field operand matrix canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "struct-field operand-matrix canary",
        "the indexed struct field should compute correctly in all operand positions",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// Array-of-structs element field as a binary operand through a BY-VALUE param.

#[test]
fn runtime_struct_field_operand_param_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_STRUCT_FIELD_OPERAND_PARAM_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-sf-operand-param-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("struct-field operand param canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "struct-field parameter-operand canary",
        "the indexed struct field should materialize through the by-value parameter",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// BOTH-RUNTIME double-indexed reads (`grid[i][j]`): machine/let targets,
// frame/machine/mixed index regions, const-prefix 3D face.

#[test]
fn runtime_double_indexed_read_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DOUBLE_INDEXED_READ_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-double-indexed-read-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("double-indexed read canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "double-indexed read canary",
        "both runtime indices should select the right elements across all storage faces",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// Deep const-prefix (`cube[1][1][k]`) + the stacked-index alias landmine
// (unit-length inner arrays, where the byte gate can't catch the swallow).

#[test]
fn runtime_nested_deep_const_prefix_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_DEEP_CONST_PREFIX_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-nested-deep-const-prefix-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("nested deep const-prefix canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "nested deep constant-prefix canary",
        "the deep constant prefix and runtime leaf should preserve their full index stack",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// DUAL-indexed copy with BOTH indices FRAME-resident params.

#[test]
fn runtime_dual_frame_index_copy_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DUAL_FRAME_INDEX_COPY_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-dual-fi-copy-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("dual frame-index copy canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "dual frame-index copy canary",
        "the two frame-resident indices should copy between the selected machine elements",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_frame_mixed_index_pair_copy_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_FRAME_MIXED_INDEX_PAIR_COPY_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-frame-mi-pair-copy-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("mixed-index frame pair-copy canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "mixed-index frame pair-copy canary",
        "mixed machine/frame indices should copy complete frame-inline aggregates",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_cross_region_indexed_pair_copy_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CROSS_REGION_INDEXED_PAIR_COPY_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-cross-region-indexed-pair-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("cross-region indexed-pair canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "cross-region indexed-pair canary",
        "machine and frame arrays should exchange complete indexed aggregate values",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_cross_region_double_indexed_pair_copy_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CROSS_REGION_DOUBLE_INDEXED_PAIR_COPY_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-cross-region-double-indexed-pair-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("cross-region double-indexed-pair canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "cross-region double-indexed pair canary",
        "machine and frame 2D arrays should exchange complete aggregate values",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn constant_nested_index_guard_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::CONSTANT_NESTED_INDEX_GUARD_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-constant-nested-index-guard-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("constant nested-index guard canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "constant nested-index guard canary",
        "the constant nested-index guard should read the authored element",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// DUAL-indexed copies with MIXED index regions (frame/machine on opposite
// sides, both directions).

#[test]
fn runtime_dual_mixed_index_copy_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DUAL_MIXED_INDEX_COPY_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-dual-mi-copy-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("dual mixed-index copy canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "dual mixed-index copy canary",
        "the opposing frame/machine index pairs should select the intended elements",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// std::time receiverless type-scoped constructors deliver a 16-byte Duration
// natively. The milliseconds path narrows a compiler-elided ranged local
// through a nested named conversion, pinning outer-argument alias composition.
// from_seconds(2)={2,0}, from_milliseconds(3500)={3,500000000} -> exit 70.
