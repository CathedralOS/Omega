use super::assert_native_exit_code;
use super::fixture_roster;
use crate::{
    compile_rooted_canary_for_native_host, compile_rooted_canary_for_target, fs, pass_canary,
};

#[test]
fn runtime_guarded_runtime_index_increment_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_GUARDED_RUNTIME_INDEX_INCREMENT_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-guarded-rt-idx-inc-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("guarded runtime-index increment canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "guarded runtime-index increment canary",
        "the guarded indexed increment should prove and update its element",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// The accumulate-into-array keystone: a dominating guard `tallies[1] < 16`
// proves the element increment `tallies[1] = tallies[1] + 1` into the element
// range [0..=16] -- the structural matcher now compares INDEXED places. -> 1.

#[test]
fn runtime_guarded_element_increment_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_GUARDED_ELEMENT_INCREMENT_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-guarded-elem-inc-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("guarded element increment canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "guarded element-increment canary",
        "the constant-index increment should prove into its element range",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// ARRAY-ELEMENT RANGES: `cells: [i32 [0..=7]; 4]` -- writes (const + runtime
// index) collect bounded obligations, ZII requires 0 in the element range, and
// an indexed READ carries the range so `cells[i] * 2 + 1` proves into
// `next: [0..=15]` with no guard (the grid-dataflow de-Trapping wall). -> 15.

#[test]
fn runtime_element_range_dataflow_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_ELEMENT_RANGE_DATAFLOW_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-elem-range-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("element range dataflow canary should compile");
    assert_native_exit_code(
        &compilation,
        15,
        "element-range dataflow canary",
        "the indexed element range should prove the derived value into [0..=15]",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// MULTI-predecessor edge agreement: two states funnel into `push` under
// structurally IDENTICAL guards `sp >= 0 && sp < 16`, proving the guarded copy
// into y: [0..=15]. The proof side's guard-equivalence walker gained literal
// leaf arms; validation joins all incoming edge envs per-place. sp=7 -> 7.

#[test]
fn runtime_funnel_guard_agreement_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_FUNNEL_GUARD_AGREEMENT_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-funnel-guard-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("funnel guard agreement canary should compile");
    assert_native_exit_code(
        &compilation,
        7,
        "funnel guard-agreement canary",
        "identical incoming guards should prove the narrowed funnel copy",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A binary value with a GUARD-bounded operand: `self.y = self.p + self.dir`,
// `p: [0..=8]` declared, `dir` bounded only by the sole incoming edge guard
// `dir >= 0 && dir <= 1`. Validation seeds the target state's env from the
// sole incoming guard (splitting `&&`); the proof side refolds the binary
// operand-wise with the guard filling the unranged operand. p=8, dir=1 -> 9.

#[test]
fn runtime_guarded_binary_operand_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_GUARDED_BINARY_OPERAND_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-guarded-bin-operand-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let host_scratch = scratch.join("host");
    let compilation = compile_rooted_canary_for_native_host(&canary, host_scratch.clone())
        .expect("guarded binary operand canary should compile");
    assert_native_exit_code(
        &compilation,
        9,
        "guarded binary-operand canary",
        "the declared and guard-bounded operands should prove their sum into [0..=9]",
    );

    let arm_scratch = scratch.join("linux-arm64");
    compile_rooted_canary_for_target(&canary, arm_scratch, "linux_arm64")
        .expect("guarded direct binary write should cross-compile for linux_arm64");
    let _ = fs::remove_dir_all(&scratch);
}

// The guarded-COPY narrowing: an UNRANGED `yv` copied into `y: [0..=9]` under
// the dominating edge guard `yv >= 0 && yv <= 9`. The checker used to bail
// before consulting the guard (guards could only refine an existing declared
// range, never establish one). yv=7 -> exit 7.

#[test]
fn runtime_guarded_copy_narrowing_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_GUARDED_COPY_NARROWING_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-guarded-copy-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("guarded copy narrowing canary should compile");
    assert_native_exit_code(
        &compilation,
        7,
        "guarded copy-narrowing canary",
        "the incoming guard should establish the copied value's target range",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// The proof-side range fold folds DIVISION and MODULO through a chain:
// `(c / 26) % 5` with `c: [0..=259]` proves into `y: [0..=4]` with no guard
// (corner-quotient divide -> [0..=9], modulo -> [0..=4]). c=259 -> exit 4.

#[test]
fn runtime_ranged_divide_modulo_chain_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_RANGED_DIVIDE_MODULO_CHAIN_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-ranged-divmod-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("ranged divide/modulo chain canary should compile");
    assert_native_exit_code(
        &compilation,
        4,
        "ranged divide/modulo-chain canary",
        "the ranged divide then modulo chain should prove into [0..=4]",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// The proof-side range fold folds BITWISE-AND over provably non-negative
// operands: `c & 15` with `c: [0..=259]` lands in [0, 15]. 259 & 15 = 3.

#[test]
fn runtime_ranged_bitwise_and_mask_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_RANGED_BITWISE_AND_MASK_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-ranged-andmask-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("ranged bitwise-and mask canary should compile");
    assert_native_exit_code(
        &compilation,
        3,
        "ranged bitwise-and-mask canary",
        "the nonnegative ranged mask should prove into [0..=15]",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A DECLARED range discharges the index obligation with no guard: `i: usize
// [0..=4]` indexing `[i32; 5]` proves both bounds (Exact-domain ranges are
// store-enforced invariants). Read face: ZII i=0 -> arr[0]=30 -> exit 30.

#[test]
fn runtime_declared_range_index_read_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DECLARED_RANGE_INDEX_READ_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-range-idx-read-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("declared range index read canary should compile");
    assert_native_exit_code(
        &compilation,
        30,
        "declared-range index-read canary",
        "the declared index range should prove an unguarded array read",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A runtime-indexed read of a narrow signed array at a nonzero offset: the
// element address joins the array base, and the i32 load is sign-restored.
// values[2] = -7 -> exit 70.

#[test]
fn runtime_signed_element_offset_read_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SIGNED_ELEMENT_OFFSET_READ_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-signed-elem-read-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("signed element offset read canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "signed element offset read canary",
        "the indexed read should address the array field and restore its sign",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// The WRITE face of the declared-range index proof: `self.arr[self.i] = 30`
// with `i: usize [0..=4]` and no dominating guard -> read-back -> exit 30.

#[test]
fn runtime_declared_range_index_write_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DECLARED_RANGE_INDEX_WRITE_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-range-idx-write-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("declared range index write canary should compile");
    assert_native_exit_code(
        &compilation,
        30,
        "declared-range index-write canary",
        "the declared index range should prove an unguarded array write and readback",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A range constraint with a CONSTANT-EXPRESSION bound: `x: i32 [0 - 1..=40]`
// folds to `[-1..=40]` via the expression-table const-eval. Expression bounds
// used to parse but silently behave UNBOUNDED (a store of 100 passed). This
// valid store of 40 must compile and run -> exit 40; the out-of-range store and
// the non-constant bound are the fail-canary twins.

#[test]
fn runtime_expression_range_bound_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_EXPRESSION_RANGE_BOUND_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-expr-range-bound-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("expression range bound canary should compile");
    assert_native_exit_code(
        &compilation,
        40,
        "expression range-bound canary",
        "the constant-expression range bound should admit its endpoint store",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// Machine-collection element-field RMW at a RUNTIME index under a dominating
// guard: `cells[k].v = cells[k].v + 1` with `v: [0..=9]` and `cells[k].v < 9`.
// The hoisted field-typed read + the guard + the machine-indexed write
// compose. cells[2].v: 4 -> 5 -> exit 1.

#[test]
fn runtime_indexed_struct_field_rmw_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_STRUCT_FIELD_RMW_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-idx-sf-rmw-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("indexed struct field rmw canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "indexed struct-field read-modify-write canary",
        "the guarded read-modify-write should store through the indexed element field",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// An array-of-structs element FIELD as a BINARY OPERAND
// (`self.x = self.cells[self.k].v + 5`): the member-over-indexed hoist + the
// field-typed temp + the machine-indexed materialization compose. -> exit 1.

#[test]
fn runtime_indexed_struct_field_operand_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_STRUCT_FIELD_OPERAND_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-idx-sf-operand-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("indexed struct field operand canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "indexed struct-field operand canary",
        "the indexed element field should materialize as a binary operand",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A MACHINE-owned array's runtime-indexed read as a transition ARGUMENT
// (`self.report(self.arr[self.k])`): used to silently pass a stale/zero
// parameter (wrong arm; interp right). Now lowered via the machine-indexed
// copy into the parameter slot. arr[2]=9, k=2 -> exit 1.

#[test]
fn runtime_machine_indexed_arg_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MACHINE_INDEXED_ARG_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-machine-indexed-arg-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("machine indexed arg canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "machine-indexed argument canary",
        "the runtime-indexed machine array value should reach the transition argument",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// The STRUCT-FIELD face: `self.report(self.cells[self.k].v)` -- an
// array-of-structs element's field as a transition argument. -> exit 1.

#[test]
fn runtime_machine_indexed_struct_field_arg_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MACHINE_INDEXED_STRUCT_FIELD_ARG_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-machine-indexed-sfa-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("machine indexed struct field arg canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "machine-indexed struct-field argument canary",
        "the runtime-indexed element field should reach the transition argument",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A FRAME-resident (by-value param) inline array read at a RUNTIME index
// (`let v = arr[k]`): used to silently read 0 (interp right). Lowered via
// CopyRuntimeFrameBaseIndexedToRuntimeFrame. arr=[10,20,30], k=1 -> exit 1.

#[test]
fn runtime_frame_indexed_param_read_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_FRAME_INDEXED_PARAM_READ_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-frame-idx-param-read-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("frame indexed param read canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "frame-indexed parameter-read canary",
        "a runtime-indexed by-value array parameter should read the selected element",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// The param-array runtime-indexed read as a BINARY OPERAND (`vals[k] + 100`)
// and as a transition ARGUMENT (`self.report(vals[k])`), with ELEMENT RANGES
// on the param type discharging the exact-arithmetic obligation. -> exit 1.

#[test]
fn runtime_frame_indexed_param_operand_arg_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_FRAME_INDEXED_PARAM_OPERAND_ARG_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-frame-idx-param-opa-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("frame indexed param operand/arg canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "frame-indexed parameter operand/argument canary",
        "the indexed parameter element should materialize as both operand and argument",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// Element-FIELD read of a by-value struct-array param at a runtime index
// (`points[k].y` -- field_byte_offset in the frame-base-indexed copy). -> 1.

#[test]
fn runtime_frame_indexed_param_field_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_FRAME_INDEXED_PARAM_FIELD_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-frame-idx-param-field-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("frame indexed param field canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "frame-indexed parameter-field canary",
        "the indexed struct parameter should preserve its selected field offset",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// Inline LOCAL copy and immediate writes at a machine-field runtime index.

#[test]
fn runtime_frame_indexed_local_read_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_FRAME_INDEXED_LOCAL_READ_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-frame-idx-local-read-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("frame indexed local read canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "frame-indexed local-read canary",
        "the inline local copy should preserve its runtime-indexed reads and writes",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// 1-byte elements (i8) of a by-value param array at a runtime index --
// byte_count 1 through the frame-base-indexed copy. small[3]=9, k=3 -> 1.

#[test]
fn runtime_frame_indexed_byte_param_read_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_FRAME_INDEXED_BYTE_PARAM_READ_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-frame-idx-byte-read-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("frame indexed byte param read canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "frame-indexed byte-parameter canary",
        "the one-byte indexed parameter element should preserve its byte width",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A VALUE-machine reading a MEMBER array of its by-value struct param at a
// runtime index (`worker.find(bx, 1)` reading `container.items[k].id`) --
// the dungeon lookup / task #15 shape; the member-of-slot branch walks
// `container -> items` to the array field's prefix offset. -> exit 1.

#[test]
fn runtime_value_machine_param_array_index_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_MACHINE_PARAM_ARRAY_INDEX_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-vm-param-arr-idx-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("value machine param array index canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "value-machine parameter-array index canary",
        "the value machine should read the runtime-indexed member-array field",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// MACHINE-owned array READ at a FRAME-resident (param) index -- the machine-
// indexed copy encoder's frame-index face (second frame-base relocation).
