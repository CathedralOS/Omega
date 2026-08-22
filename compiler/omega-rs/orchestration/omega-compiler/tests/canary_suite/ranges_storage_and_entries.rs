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
fn runtime_guarded_runtime_index_increment_exit_canary_runs() {
    let canary = pass_canary("range/runtime_guarded_runtime_index_increment_exit");
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
    let canary = pass_canary("range/runtime_guarded_element_increment_exit");
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
    let canary = pass_canary("range/runtime_element_range_dataflow_exit");
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
    let canary = pass_canary("range/runtime_funnel_guard_agreement_exit");
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
    let canary = pass_canary("range/runtime_guarded_binary_operand_exit");
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
    let canary = pass_canary("range/runtime_guarded_copy_narrowing_exit");
    let scratch = std::env::temp_dir().join(format!("omega-guarded-copy-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("guarded copy narrowing canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("guarded copy narrowing canary should run");

    assert_eq!(
        output.status.code(),
        Some(7),
        "expected the edge-guarded copy `self.y = self.yv` (guard establishes \
         [0..=9]) to prove and exit 7, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// The proof-side range fold folds DIVISION and MODULO through a chain:
// `(c / 26) % 5` with `c: [0..=259]` proves into `y: [0..=4]` with no guard
// (corner-quotient divide -> [0..=9], modulo -> [0..=4]). c=259 -> exit 4.
#[test]
fn runtime_ranged_divide_modulo_chain_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_ranged_divide_modulo_chain_exit");
    let scratch = std::env::temp_dir().join(format!("omega-ranged-divmod-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("ranged divide/modulo chain canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("ranged divide/modulo chain canary should run");

    assert_eq!(
        output.status.code(),
        Some(4),
        "expected `(self.c / 26) % 5` (c=259, ranged [0..=259]) to prove into \
         y: [0..=4] and exit 4, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// The proof-side range fold folds BITWISE-AND over provably non-negative
// operands: `c & 15` with `c: [0..=259]` lands in [0, 15]. 259 & 15 = 3.
#[test]
fn runtime_ranged_bitwise_and_mask_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_ranged_bitwise_and_mask_exit");
    let scratch = std::env::temp_dir().join(format!("omega-ranged-andmask-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("ranged bitwise-and mask canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("ranged bitwise-and mask canary should run");

    assert_eq!(
        output.status.code(),
        Some(3),
        "expected `self.c & 15` (c=259, ranged) to prove into y: [0..=15] and \
         exit 259 & 15 = 3, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A DECLARED range discharges the index obligation with no guard: `i: usize
// [0..=4]` indexing `[i32; 5]` proves both bounds (Exact-domain ranges are
// store-enforced invariants). Read face: ZII i=0 -> arr[0]=30 -> exit 30.
#[test]
fn runtime_declared_range_index_read_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_declared_range_index_read_exit");
    let scratch = std::env::temp_dir().join(format!("omega-range-idx-read-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("declared range index read canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("declared range index read canary should run");

    assert_eq!(
        output.status.code(),
        Some(30),
        "expected `self.arr[self.i]` (i: usize [0..=4], no guard) to prove and read \
         arr[0]=30 -> exit 30, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// The WRITE face of the declared-range index proof: `self.arr[self.i] = 30`
// with `i: usize [0..=4]` and no dominating guard -> read-back -> exit 30.
#[test]
fn runtime_declared_range_index_write_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_declared_range_index_write_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-range-idx-write-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("declared range index write canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("declared range index write canary should run");

    assert_eq!(
        output.status.code(),
        Some(30),
        "expected `self.arr[self.i] = 30` (i: usize [0..=4], no guard) to prove, \
         write arr[0], and read back 30 -> exit 30, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
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
    let canary = pass_canary("arithmetic/runtime_expression_range_bound_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-expr-range-bound-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("expression range bound canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("expression range bound canary should run");

    assert_eq!(
        output.status.code(),
        Some(40),
        "expected the store of 40 into `x: i32 [0 - 1..=40]` (folded to -1..=40) to \
         compile and run -> exit 40, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// Machine-collection element-field RMW at a RUNTIME index under a dominating
// guard: `cells[k].v = cells[k].v + 1` with `v: [0..=9]` and `cells[k].v < 9`.
// The hoisted field-typed read + the guard + the machine-indexed write
// compose. cells[2].v: 4 -> 5 -> exit 1.
#[test]
fn runtime_indexed_struct_field_rmw_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_indexed_struct_field_rmw_exit");
    let scratch = std::env::temp_dir().join(format!("omega-idx-sf-rmw-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("indexed struct field rmw canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("indexed struct field rmw canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `cells[k].v = cells[k].v + 1` (4 -> 5) to store through the          element field and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// An array-of-structs element FIELD as a BINARY OPERAND
// (`self.x = self.cells[self.k].v + 5`): the member-over-indexed hoist + the
// field-typed temp + the machine-indexed materialization compose. -> exit 1.
#[test]
fn runtime_indexed_struct_field_operand_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_indexed_struct_field_operand_exit");
    let scratch = std::env::temp_dir().join(format!("omega-idx-sf-operand-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("indexed struct field operand canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("indexed struct field operand canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `self.cells[self.k].v + 5` (cells[2].v=9) to compute 14 and          exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A MACHINE-owned array's runtime-indexed read as a transition ARGUMENT
// (`self.report(self.arr[self.k])`): used to silently pass a stale/zero
// parameter (wrong arm; interp right). Now lowered via the machine-indexed
// copy into the parameter slot. arr[2]=9, k=2 -> exit 1.
#[test]
fn runtime_machine_indexed_arg_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_machine_indexed_arg_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-machine-indexed-arg-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("machine indexed arg canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("machine indexed arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `self.report(self.arr[self.k])` (arr[2]=9, k=2) to pass 9 and          exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// The STRUCT-FIELD face: `self.report(self.cells[self.k].v)` -- an
// array-of-structs element's field as a transition argument. -> exit 1.
#[test]
fn runtime_machine_indexed_struct_field_arg_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_machine_indexed_struct_field_arg_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-machine-indexed-sfa-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("machine indexed struct field arg canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("machine indexed struct field arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `self.report(self.cells[self.k].v)` (cells[2].v=9, k=2) to pass          9 and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A FRAME-resident (by-value param) inline array read at a RUNTIME index
// (`let v = arr[k]`): used to silently read 0 (interp right). Lowered via
// CopyRuntimeFrameBaseIndexedToRuntimeFrame. arr=[10,20,30], k=1 -> exit 1.
#[test]
fn runtime_frame_indexed_param_read_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_frame_indexed_param_read_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-frame-idx-param-read-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("frame indexed param read canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("frame indexed param read canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `let v = arr[k]` (arr=[10,20,30], k=1) to read 20 and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// The param-array runtime-indexed read as a BINARY OPERAND (`vals[k] + 100`)
// and as a transition ARGUMENT (`self.report(vals[k])`), with ELEMENT RANGES
// on the param type discharging the exact-arithmetic obligation. -> exit 1.
#[test]
fn runtime_frame_indexed_param_operand_arg_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_frame_indexed_param_operand_arg_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-frame-idx-param-opa-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("frame indexed param operand/arg canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("frame indexed param operand/arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `vals[k] + 100` == 130 then `self.report(vals[k])` == 30 to exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// Element-FIELD read of a by-value struct-array param at a runtime index
// (`points[k].y` -- field_byte_offset in the frame-base-indexed copy). -> 1.
#[test]
fn runtime_frame_indexed_param_field_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_frame_indexed_param_field_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-frame-idx-param-field-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("frame indexed param field canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("frame indexed param field canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `points[k].y` (points[1].y=42, k=1) to read 42 and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// Inline LOCAL copy and immediate writes at a machine-field runtime index.
#[test]
fn runtime_frame_indexed_local_read_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_frame_indexed_local_read_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-frame-idx-local-read-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("frame indexed local read canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("frame indexed local read canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected machine-indexed inline-frame copy, immediate, and binary writes to read back and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// 1-byte elements (i8) of a by-value param array at a runtime index --
// byte_count 1 through the frame-base-indexed copy. small[3]=9, k=3 -> 1.
#[test]
fn runtime_frame_indexed_byte_param_read_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_frame_indexed_byte_param_read_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-frame-idx-byte-read-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("frame indexed byte param read canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("frame indexed byte param read canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `small[k]` (i8, small[3]=9, k=3) to read 9 and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A VALUE-machine reading a MEMBER array of its by-value struct param at a
// runtime index (`worker.find(bx, 1)` reading `container.items[k].id`) --
// the dungeon lookup / task #15 shape; the member-of-slot branch walks
// `container -> items` to the array field's prefix offset. -> exit 1.
#[test]
fn runtime_value_machine_param_array_index_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_value_machine_param_array_index_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-vm-param-arr-idx-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("value machine param array index canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("value machine param array index canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `container.items[k].id` (items[1].id=42, k=1) through the value          machine to read 42 and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// MACHINE-owned array READ at a FRAME-resident (param) index -- the machine-
// indexed copy encoder's frame-index face (second frame-base relocation).
#[test]
fn runtime_machine_frame_index_read_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_machine_frame_index_read_exit");
    let scratch = std::env::temp_dir().join(format!("omega-mfi-read-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("machine frame-index read canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("machine frame-index read canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `self.arr[k]` (arr[2]=9, k=2 a param) to read 9 and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// MACHINE-owned array WRITE of a runtime value at a FRAME-resident index
// (machine source + frame index).
#[test]
fn runtime_machine_frame_index_write_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_machine_frame_index_write_exit");
    let scratch = std::env::temp_dir().join(format!("omega-mfi-write-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("machine frame-index write canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("machine frame-index write canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `self.arr[k] = self.b` (b=7, k=2) to store 7 and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// MACHINE-owned array WRITE with BOTH source and index frame-resident params
// -- the three-relocation case.
#[test]
fn runtime_machine_frame_index_dual_frame_write_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_machine_frame_index_dual_frame_write_exit");
    let scratch = std::env::temp_dir().join(format!("omega-mfi-dual-write-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("machine frame-index dual-frame write canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("machine frame-index dual-frame write canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `self.arr[k] = v` (k=1, v=44, both params) to store 44 and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// MACHINE-array RMW at a FRAME (param) index under a dominating guard: the
// frame-index BINARY write encoder + the whole-machine param scope in the
// index prover (k's declared range reaches the sub-state).
#[test]
fn runtime_machine_frame_index_rmw_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_machine_frame_index_rmw_exit");
    let scratch = std::env::temp_dir().join(format!("omega-mfi-rmw-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("machine frame-index rmw canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("machine frame-index rmw canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `self.arr[k] = self.arr[k] + 1` (arr[2]: 4 -> 5, k a param) to exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// Machine-array reads at FRAME indices as binary operands (incl. a struct
// element field) and as a transition argument.
#[test]
fn runtime_machine_frame_index_arg_operand_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_machine_frame_index_arg_operand_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-mfi-arg-operand-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("machine frame-index arg/operand canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("machine frame-index arg/operand canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `self.arr[k] + self.cells[j].v` == 39 then `self.report(self.arr[k])` to exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// CONST-row + RUNTIME-column 2D reads: machine-field AND frame-let consumers.
#[test]
fn runtime_nested_const_row_indexed_read_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_nested_const_row_indexed_read_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-nested-const-row-read-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("nested const-row indexed read canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("nested const-row indexed read canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `self.grid[1][j]` (j=2 runtime, grid[1][2]=42) to read 42 through both the machine-field and let consumers and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// CONST-element + RUNTIME-leaf struct-field array WRITE, neighbor-validated.
#[test]
fn runtime_nested_const_row_struct_field_write_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_nested_const_row_struct_field_write_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-nested-const-row-sf-write-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("nested const-row struct-field write canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("nested const-row struct-field write canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `self.rows[2].data[j] = 77` (j=1 runtime) to write the right element (neighbors untouched) and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// 3D RUNTIME-MIDDLE index (`cube[1][b][0]`): const leaf rides the suffix walk
// above the runtime level, const prefix folds into the collection resolution.
#[test]
fn runtime_nested_middle_index_3d_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_nested_middle_index_3d_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-nested-middle-3d-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("nested runtime-middle 3D canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("nested runtime-middle 3D canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `cube[1][b][0]` read+write (b=0 runtime) to hit the right element and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A range-typed LET carrying a computed index (`let m = k + 1; arr[m]`) on
// every face: read, write target, guard subject, backward offset, bare-copy.
#[test]
fn runtime_let_bound_computed_index_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_let_bound_computed_index_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-let-computed-index-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("let-bound computed index canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("let-bound computed index canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `let m = k + 1; arr[m]` faces (read/write/guard/backward/copy) to hit the right elements and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// Array-of-structs element field in every binary-operand position (left/right
// operand, both-indexed, guard-dominated RMW, guard subject, indexed target).
#[test]
fn runtime_struct_field_operand_matrix_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_struct_field_operand_matrix_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-sf-operand-matrix-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("struct-field operand matrix canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("struct-field operand matrix canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `cells[i].x` to compute correctly in all six operand positions and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// Array-of-structs element field as a binary operand through a BY-VALUE param.
#[test]
fn runtime_struct_field_operand_param_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_struct_field_operand_param_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-sf-operand-param-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("struct-field operand param canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("struct-field operand param canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `cells[i].x + 5` through a by-value param array to compute 42 and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// BOTH-RUNTIME double-indexed reads (`grid[i][j]`): machine/let targets,
// frame/machine/mixed index regions, const-prefix 3D face.
#[test]
fn runtime_double_indexed_read_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_double_indexed_read_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-double-indexed-read-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("double-indexed read canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("double-indexed read canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `grid[i][j]` (both indices runtime) to read the right elements across all faces and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// Deep const-prefix (`cube[1][1][k]`) + the stacked-index alias landmine
// (unit-length inner arrays, where the byte gate can't catch the swallow).
#[test]
fn runtime_nested_deep_const_prefix_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_nested_deep_const_prefix_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-nested-deep-const-prefix-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("nested deep const-prefix canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("nested deep const-prefix canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `cube[1][1][k]` read+write and the unit-length alias shape (`weird[1][2][z]`) to hit the right elements and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// DUAL-indexed copy with BOTH indices FRAME-resident params.
#[test]
fn runtime_dual_frame_index_copy_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_dual_frame_index_copy_exit");
    let scratch = std::env::temp_dir().join(format!("omega-dual-fi-copy-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("dual frame-index copy canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("dual frame-index copy canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `self.arr[k] = self.arr[j]` (k=3, j=1 params, arr[1]=77) to exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_frame_mixed_index_pair_copy_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_frame_mixed_index_pair_copy_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-frame-mi-pair-copy-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("mixed-index frame pair-copy canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("mixed-index frame pair-copy canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected mixed machine/frame indices to copy complete frame-inline aggregate values, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_cross_region_indexed_pair_copy_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_cross_region_indexed_pair_copy_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-cross-region-indexed-pair-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("cross-region indexed-pair canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("cross-region indexed-pair canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected machine/frame indexed arrays to exchange complete aggregate values, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_cross_region_double_indexed_pair_copy_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_cross_region_double_indexed_pair_copy_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-cross-region-double-indexed-pair-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("cross-region double-indexed-pair canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("cross-region double-indexed-pair canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected machine/frame 2D arrays to exchange complete aggregate values, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn constant_nested_index_guard_exit_canary_runs() {
    let canary = pass_canary("collections/constant_nested_index_guard_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-constant-nested-index-guard-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("constant nested-index guard canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("constant nested-index guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected a constant nested-index guard to read the authored element, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// DUAL-indexed copies with MIXED index regions (frame/machine on opposite
// sides, both directions).
#[test]
fn runtime_dual_mixed_index_copy_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_dual_mixed_index_copy_exit");
    let scratch = std::env::temp_dir().join(format!("omega-dual-mi-copy-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("dual mixed-index copy canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("dual mixed-index copy canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `arr[k]=arr[self.j]` then `arr[self.j]=arr[m]` to land on the right          elements and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// std::time receiverless type-scoped constructors deliver a 16-byte Duration
// natively. The milliseconds path narrows a compiler-elided ranged local
// through a nested named conversion, pinning outer-argument alias composition.
// from_seconds(2)={2,0}, from_milliseconds(3500)={3,500000000} -> exit 70.
#[test]
fn runtime_duration_constructors_exit_canary_runs() {
    let canary = pass_canary("time/runtime_duration_constructors_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-duration-ctors-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("duration constructors canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("duration constructors canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected from_seconds(2) + from_milliseconds(3500) to deliver exact fields and exit 70, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Slice-descriptor element read/write/RMW against MACHINE storage -- the
// fixed-index read was width-0 (silently dropped) on x86_64. -> exit 1.
#[test]
fn runtime_slice_element_machine_roundtrip_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_slice_element_machine_roundtrip_exit");
    let scratch = std::env::temp_dir().join(format!("omega-slice-elem-rt-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("slice element machine roundtrip canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("slice element machine roundtrip canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected s[1] read == 11, s[2] write == 77, s[0] RMW == 5 to exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// RUNTIME-index slice-descriptor element read into a machine field. -> 1.
#[test]
fn runtime_slice_element_runtime_index_read_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_slice_element_runtime_index_read_exit");
    let scratch = std::env::temp_dir().join(format!("omega-slice-elem-ri-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("slice element runtime index read canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("slice element runtime index read canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `self.got = s[i]` (i=1, s[1]=11) to read 11 and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A MEMBER-expression value-call arg (`self.grab(self.bx.inner)`) with a
// NESTED member read in the callee (`b.value`): the alias substitution's
// suffix must survive the member-rooted receiver (the suffix-drop class).
// pad=99/value=42 discriminate offset-0 reads. -> exit 1.
#[test]
fn runtime_member_arg_nested_read_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_member_arg_nested_read_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-member-arg-nested-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("member arg nested read canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("member arg nested read canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected b.value == 42 (not pad's 99 at offset 0) to exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Type-scoped constructor with COMPUTED struct-literal fields delivers
// natively (the member-suffix drop in append_place_suffix is fixed). -> 1.
#[test]
fn runtime_constructor_computed_field_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_constructor_computed_field_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-ctor-computed-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("constructor computed field canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("constructor computed field canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected Pair8::make(3500) == {{35, 7}} (per-field delivery, no root clobber) to exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// MACHINE-FIELD-bounded subslice local (`self.arr[self.lo..self.hi]`) --
// the START's indexed-address op is region-tagged now. len 3 -> exit 3.
#[test]
fn runtime_machine_bounded_subslice_local_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_machine_bounded_subslice_local_exit");
    let scratch = std::env::temp_dir().join(format!("omega-mach-subslice-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("machine-bounded subslice canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("machine-bounded subslice canary should run");

    assert_eq!(
        output.status.code(),
        Some(3),
        "expected `self.arr[self.lo..self.hi]` (1..4) len 3 to exit 3, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// Runtime-START subslice POINTER correctness (the ptr write was width-0 and
// silently dropped on x86_64; only len checks passed). s[0]==arr[1] -> 1.
#[test]
fn runtime_subslice_start_pointer_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_subslice_start_pointer_exit");
    let scratch = std::env::temp_dir().join(format!("omega-subslice-ptr-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("subslice start pointer canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("subslice start pointer canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected sub[0] == arr[1] == 10 (the shrunk pointer, not the array base) to exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// LOOP-CARRIED accumulator through the dispatch self-loop: transition args
// stage through scratch so `acc + n` reads the pre-decrement n. -> 15 -> 1.
#[test]
fn runtime_loop_accumulator_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_loop_accumulator_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-rec-accum-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("recursive accumulator canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("recursive accumulator canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected sum(5,0) == 15 (parallel-assignment staging) to exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// 3-arg ROTATION through a self-transition loop -- the full parallel-assignment
// cycle (`-> rot(k-1, b, c, a)`). rot(3,1,2,3) -> a==1 -> exit 1.
#[test]
fn runtime_loop_rotation_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_loop_rotation_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-rec-rot-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("recursive rotation canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("recursive rotation canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected rot(3,1,2,3) to rotate back to a==1 and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// `-> usize` written AFTER the machine clauses (`terminates -> usize`): the
// parser used to silently DROP it (skip-any-token fallback), so the machine
// parsed as VOID and callers bound ZII 0. Non-recursive value flows -> exit 1.
#[test]
fn runtime_post_clauses_return_type_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_post_clauses_return_type_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-post-clauses-ret-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("post-clauses return type canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("post-clauses return type canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `terminates -> usize` pick(1) to return 5 and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A runtime slice `.len` read into a LOCAL binding in a VALUE position (`let n =
// s.len`), NOT as an operand or guard subject. The native side used to leave the
// local slot unwritten (the descriptor length was never materialized), so a later
// `n == 5` guard read the zeroed slot and took the false arm -- a silent read-0
// miscompile the interpreter never had. `s` views a fixed `[i32; 5]` through
// `.as_slice()`, so the length folds to 5 and the guard matches -> exit 5.
#[test]
fn runtime_slice_length_local_binding_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_slice_length_local_binding_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-slice-len-local-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("slice length local binding canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("slice length local binding canary should run");

    assert_eq!(
        output.status.code(),
        Some(5),
        "expected `let n = s.len` (as_slice-local) to materialize the length so \
         `n == 5` matches -> exit 5, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A runtime slice PARAM `.len` read into a LOCAL `usize` binding (`let n = s.len`).
// The `.len` place resolver reports the descriptor's low 4-byte len word, so a
// wider 8-byte `usize` slot failed the exact-size copy and the write was dropped
// (the local read 0). The descriptor holds the full 8-byte len, now read at the
// target's width. `s` views a fixed `[i32; 6]`, so `n == 6` matches -> exit 6.
#[test]
fn runtime_slice_length_local_param_binding_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_slice_length_local_param_binding_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-slice-len-param-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("slice length local param binding canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("slice length local param binding canary should run");

    assert_eq!(
        output.status.code(),
        Some(6),
        "expected `let n: usize = s.len` (slice param) to read the descriptor's \
         full 8-byte length so `n == 6` matches -> exit 6, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A literal-bounded SUBSLICE `.len` read into a LOCAL binding in a value position
// (`let sub = self.arr[1..4]; let n = sub.len`). The subslice binding folds to the
// inline `(self.arr[1..4]).len` (no runtime descriptor slot), which the value-write
// resolver used to drop -> the local read 0. The window length `4 - 1 = 3` is a
// compile-time constant, now folded in the value-write path. `n == 3` matches -> 3.
#[test]
fn runtime_subslice_length_local_binding_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_subslice_length_local_binding_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-subslice-len-local-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("subslice length local binding canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("subslice length local binding canary should run");

    assert_eq!(
        output.status.code(),
        Some(3),
        "expected `let n = sub.len` (sub = a literal `arr[1..4]` subslice) to fold to \
         the window length 3 so `n == 3` matches -> exit 3, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The INLINE subslice `.len` (no `let sub` binding): `let n = (self.arr[1..4]).len`.
// Native folds the window length 3; the interpreter used to reject it ("range
// expression outside index position") because a member on a non-place receiver
// resolved as a place and hit the raw range -- it now evaluates the receiver as a
// value and reads `.len` off it. Both engines agree -> `n == 3` matches -> exit 3.
#[test]
fn runtime_inline_subslice_length_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_inline_subslice_length_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-inline-subslice-len-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("inline subslice length canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("inline subslice length canary should run");

    assert_eq!(
        output.status.code(),
        Some(3),
        "expected `let n = (self.arr[1..4]).len` to fold to the window length 3 so \
         `n == 3` matches -> exit 3, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A FIXED-ARRAY base subslice with a RUNTIME (machine-field) END bound to a
// LOCAL: `let sub = self.arr[1..self.hi]`. The emitter seeds the local's slot
// with the whole-array descriptor (ptr = &arr, len = declared length) and
// shrinks it IN PLACE with the runtime end. Was fenced before this lowering.
// hi=4 -> window [1,4) -> len 3 -> exit 3.
#[test]
fn runtime_end_fixed_array_subslice_local_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_end_fixed_array_subslice_local_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-rt-end-subslice-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("runtime-end fixed-array subslice local canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("runtime-end fixed-array subslice local canary should run");

    assert_eq!(
        output.status.code(),
        Some(3),
        "expected `let sub = self.arr[1..self.hi]` (hi=4) to seed-and-shrink the \
         descriptor so `sub.len == 3` matches -> exit 3, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// The ELEMENT-READ face of the fixed-array runtime-END subslice local: the
// seeded-then-shrunk descriptor's POINTER must be element 1's address, not just
// a correct length. `sub = self.arr[1..self.hi]` over 10,20,30,40,50 -> a
// guarded `sub[0]` read through the descriptor -> 20.
#[test]
fn runtime_end_fixed_array_subslice_element_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_end_fixed_array_subslice_element_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-rt-end-subslice-elem-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("runtime-end fixed-array subslice element canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("runtime-end fixed-array subslice element canary should run");

    assert_eq!(
        output.status.code(),
        Some(20),
        "expected `sub[0]` through the seeded-then-shrunk `self.arr[1..self.hi]` \
         descriptor to read element 1 (20) -> exit 20, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A machine-field FIXED array's `.len` as a dispatch-guard comparison OPERAND
// against a runtime value (`hi <= self.arr.len`): the guard-normalization fold
// rewrites the constant length to its literal -> CompareStaticValue. Previously
// it classified as a runtime compare with no right storage and died at emission.
// hi=4 vs len 5 -> true arm -> exit 7.
#[test]
fn guard_fixed_array_len_operand_exit_canary_runs() {
    let canary = pass_canary("slices/guard_fixed_array_len_operand_exit");
    let scratch = std::env::temp_dir().join(format!("omega-guard-arr-len-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("guard fixed-array len operand canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("guard fixed-array len operand canary should run");

    assert_eq!(
        output.status.code(),
        Some(7),
        "expected `hi <= self.arr.len` (hi=4, len 5) to fold the length and take \
         the true arm -> exit 7, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// The FULL runtime-bounded fixed-array subslice arc: both bounds runtime params,
// the dominating guard `lo <= hi && hi <= self.arr.len` lowers (fixed-array
// `.len` operand fold) AND discharges the prover's subslice obligations, and the
// true arm passes `self.arr[lo..hi]` as a slice argument through the
// seeded-then-shrunk descriptor. len 3 -> exit 3.
#[test]
fn runtime_bounded_fixed_array_subslice_arg_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_bounded_fixed_array_subslice_arg_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-rt-bounded-subslice-arg-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("runtime-bounded fixed-array subslice arg canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("runtime-bounded fixed-array subslice arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(3),
        "expected the guarded `self.arr[lo..hi]` (1..4) slice argument to carry \
         window length 3 -> exit 3, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// #66 owned `[u8; N] in Utf8` carrier builder/concat, native: `self.text =
// "Room " + self.label` materializes into the target carrier's inline storage --
// the first literal initializes it, then the source carrier's content is appended
// (running offset + running len). `self.text == "Room A1"` matches -> exit 70.
#[test]
fn runtime_bounded_carrier_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_concat_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-bounded-carrier-concat-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bounded carrier concat canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("bounded carrier concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the owned `[u8; 16] in Utf8` carrier to materialize `\"Room \" + self.label` \
         into its inline storage so `self.text == \"Room A1\"` exits 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// #66 owned `[u8; N] in Utf8` carrier MULTI-SEGMENT concat into a `&mut` OUT-PARAM
// (the dungeon render-line shape): `out_line = "== " + self.label + " =="` writes
// across a machine boundary into a borrowed carrier -- init literal, append the
// source carrier, append the trailing literal at the running length. Exits 70.
#[test]
fn runtime_bounded_carrier_alias_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_alias_concat_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-bounded-carrier-alias-concat-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bounded carrier alias concat canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("bounded carrier alias concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `out_line = \"== \" + self.label + \" ==\"` to materialize `== Gate ==` into the \
         borrowed carrier so `self.line == \"== Gate ==\"` exits 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// #66 owned carrier concat with a FRAME-LOCAL source: `out_line = "== " + src +
// " =="` where `src` is a `let`-local carrier read from the runtime frame base.
#[test]
fn runtime_bounded_carrier_local_source_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_local_source_concat_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-bounded-carrier-local-source-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bounded carrier local source concat canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("bounded carrier local source concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `out_line = \"== \" + src + \" ==\"` (frame-local source) to render `== Gate ==` \
         so `self.line == \"== Gate ==\"` exits 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// Carrier sibling of the slice-view-element test: a value-call guards on the
// element's CARRIER field (`room.label == "Gate"`, room = r[0], r =
// self.rooms.as_slice()). Carrier RECOGNITION now traces the elided local and
// sees through the as_slice view to resolve the field descriptor against the
// underlying array; before, the carrier `==` failed to lower (the arm was
// poisoned). Exits 70.
#[test]
fn runtime_value_call_slice_view_carrier_guard_exit_canary_runs() {
    let canary = pass_canary("text/runtime_value_call_slice_view_carrier_guard_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-value-call-slice-view-carrier-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("value-call slice-view carrier guard canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("value-call slice-view carrier guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the carrier guard `room.label == \"Gate\"` (room = r[0], r a \
         slice view) to resolve and take the true arm -> exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A value-call forwarded a SLICE-VIEW element by value (`read(r[0])`, r a local
// `self.rooms.as_slice()`): the body reads `room.id` through the BranchParameter
// alias `room = r[0]` -> `(self.rooms.as_slice())[0].id`. The place resolver now
// sees through the as_slice view AND traces the elided local to its initializer
// so the element resolves against the underlying array; before, it read a zero
// slot and the call returned 0. Exits 70.
#[test]
fn runtime_value_call_slice_view_element_arg_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_value_call_slice_view_element_arg_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-value-call-slice-view-elem-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("value-call slice-view element canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("value-call slice-view element canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `read(r[0])` (r = self.rooms.as_slice()) to resolve room.id to the \
         underlying array element and return 7 -> exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A `let`-local capturing `self.vm.sp + 1`, where `self.vm.sp` is reassigned
// before the local is forwarded through a nested dispatch (try_push1 -> push1).
// Argument materialization used to inline-fold the local back into its
// initializer and re-evaluate it AFTER the field was overwritten, so a deeper
// substate's guard saw the wrong value and branched into the wrong arm. The fix
// keeps the captured slot. Exits 70 (both pushes land: stack[0]=3, stack[1]=4).
#[test]
fn runtime_linear_search_early_exit_canary_runs() {
    // Linear search with EARLY loop exit: scan for `target`, leave the loop the instant it's found
    // (each element read into a field first, then compared). arr=[3,7,12,18,5], target=12 -> index 2.
    let canary = pass_canary("control_flow/runtime_linear_search_early_exit");
    let scratch = std::env::temp_dir().join(format!("omega-linear-search-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("linear search early exit canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("linear search early exit canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected linear search to find target 12 at index 2 and stop (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_entry_computed_result_exit_canary_runs() {
    // An ordinary value helper returns its computed terminal through result
    // scratch; the rooted Unit entry consumes it and exits explicitly.
    let canary = pass_canary("control_flow/runtime_entry_return_field_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-entry-return-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("computed helper return canary should compile");
    let footprint_artifact = fs::read_to_string(build_dir.join("08_boundary_footprints.json"))
        .expect("computed entry return footprint evidence should be written");
    assert!(
        footprint_artifact.contains("\"origin\": \"exit_result_registers\"")
            && footprint_artifact.contains("\"enumeration_complete\": false"),
        "runtime helper result load must retain result-register evidence without claiming final completeness"
    );
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("computed entry return canary should run");
    assert_eq!(
        output.status.code(),
        Some(200),
        "expected main to return computed value 200 as the exit code; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_entry_unary_result_exit_canary_runs() {
    // A runtime logical-NOT terminal computes through one-byte helper-result
    // scratch; the rooted Unit entry dispatches on the returned bool.
    let canary = pass_canary("control_flow/runtime_entry_unary_result_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-entry-unary-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime unary helper return canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime unary entry return canary should run");
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected main to return !false (exit 1); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    let cross_dir =
        std::env::temp_dir().join(format!("omega-entry-unary-arm64-{}", std::process::id()));
    let _ = fs::remove_dir_all(&cross_dir);
    let src_dir = cross_dir.join("src");
    let out_dir = cross_dir.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(
        src_dir.join("build.omg"),
        hosted_main_program_entry_build("linux_arm64"),
    )
    .expect("write target manifest");
    compile(CompileOptions {
        root_path: src_dir.join("main.omg"),
        build_dir: Some(out_dir.clone()),
        target_name: Some("linux_arm64".into()),
        write_output: true,
    })
    .expect("runtime unary entry return should cross-compile for AArch64");
    fs::read(out_dir.join("omega-program")).expect("read emitted AArch64 ELF");
    let _ = fs::remove_dir_all(&cross_dir);
}

#[test]
fn runtime_entry_cast_result_exit_canary_runs() {
    // A runtime u8-to-i32 terminal cast uses the ordinary conversion writer in
    // helper-result scratch, then returns the widened value to the Unit entry.
    let canary = pass_canary("control_flow/runtime_entry_cast_result_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-entry-cast-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime cast helper return canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime cast entry return canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected widened u8 entry result to exit 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    let cross_dir =
        std::env::temp_dir().join(format!("omega-entry-cast-arm64-{}", std::process::id()));
    let _ = fs::remove_dir_all(&cross_dir);
    let src_dir = cross_dir.join("src");
    let out_dir = cross_dir.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(
        src_dir.join("build.omg"),
        hosted_main_program_entry_build("linux_arm64"),
    )
    .expect("write target manifest");
    compile(CompileOptions {
        root_path: src_dir.join("main.omg"),
        build_dir: Some(out_dir.clone()),
        target_name: Some("linux_arm64".into()),
        write_output: true,
    })
    .expect("runtime cast entry return should cross-compile for AArch64");
    fs::read(out_dir.join("omega-program")).expect("read emitted AArch64 ELF");
    let _ = fs::remove_dir_all(&cross_dir);
}

#[test]
fn runtime_entry_nested_binary_result_exit_canary_runs() {
    // Recursive runtime value operands preserve nested arithmetic instead of
    // requiring each immediate child of the terminal binary to be a place.
    let canary = pass_canary("control_flow/runtime_entry_nested_binary_result_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-entry-nested-binary-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested binary helper return canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("nested binary entry return canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected nested arithmetic entry result to exit 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_entry_scalar_operation_results_exit_canaries_run() {
    // The shared pre-resolved scalar writer covers both builtin calls and
    // comparison-valued binaries at an entry terminal.
    for (name, expected) in [
        ("runtime_entry_builtin_result_exit", 70),
        ("runtime_entry_comparison_result_exit", 1),
    ] {
        let canary = pass_canary(&format!("control_flow/{name}"));
        let build_dir = std::env::temp_dir().join(format!("omega-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&build_dir);
        compile(CompileOptions {
            root_path: canary.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: None,
            write_output: true,
        })
        .expect("scalar-operation entry return canary should compile");
        let output = Command::new(build_dir.join(executable_name()))
            .output()
            .expect("scalar-operation entry return canary should run");
        assert_eq!(
            output.status.code(),
            Some(expected),
            "unexpected entry result for {name}; got {:?}\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn free_standing_helper_result_canary_runs() {
    let canary = pass_canary("calls/free_standing_machine_helper_compile");
    let build_dir = std::env::temp_dir().join(format!("omega-entry-helper-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("free-standing terminal helper should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("free-standing terminal helper should run");
    assert_eq!(
        output.status.code(),
        Some(7),
        "expected add_i32(3, 4) to return exit 7; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_loop_patterns_exit_canary_runs() {
    // Loop patterns via self-transition: a LARGE counting loop (1..10000) stays
    // iterative (no stack growth) and nested loops re-initialize the inner counter.
    // Guards the state-recursion lowering that serious apps lean on.
    let canary = pass_canary("control_flow/runtime_loop_patterns_exit");
    let scratch = std::env::temp_dir().join(format!("omega-loop-patterns-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("loop-patterns canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("loop-patterns canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a 10000-iteration counting loop + nested loops to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_composite_initializer_local_arg_exit_canary_runs() {
    // A let-local whose initializer is a composite (binary / unary / cast) reading a
    // prior local or field, forwarded as a transition argument. The dispatch-arg fold
    // must recurse into the composite to resolve the inner local; missing Cast/Binary/
    // Unary arms re-materialized it in the target frame (no slot) and read 0.
    let canary = pass_canary("control_flow/runtime_composite_initializer_local_arg_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-composite-initializer-arg-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("composite-initializer-local-arg canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("composite-initializer-local-arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected binary/unary/field-read composite initializers forwarded as args to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_captured_local_remutated_field_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_captured_local_remutated_field_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-captured-local-remutated-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("captured-local-remutated-field canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("captured-local-remutated-field canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the captured `new_sp` slot (not a re-folded `self.vm.sp + 1`) to \
         drive the nested dispatch so both pushes land and exit is 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// #66 carrier compare through a POINTEE in a VALUE-CALL guard: the value-call
// `Finder::check(level) -> i32` branches on `r[0].label == "Gate"` where `r:
// &[Room]` indexes the by-value `level` param, so `r[0].label` is a carrier
// reached through the slice pointer. The guard resolves the pointee place and
// lowers the bounded-buffer compare; before the fix the resolver bailed, the
// leaf branch dropped the arm write (the literal-guard poison-skip), and the
// value-call returned a stale 0. Exits 70 (the `== "Gate"` true arm).
#[test]
fn runtime_bounded_carrier_pointee_guard_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_pointee_guard_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-bounded-carrier-pointee-guard-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bounded carrier pointee guard canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("bounded carrier pointee guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the value-call guard `r[0].label == \"Gate\"` (carrier through a \
         slice-element pointee) to take the true arm and return 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// #66 owned `[u8; N] in Utf8` carrier field reached THROUGH a slice pointer:
// `cells[0].label = "Gate"` writes the carrier inline through the `&mut [Room]`
// pointer (a pointee write), then reads it back through the same pointer. Exits 70.
#[test]
fn runtime_bounded_carrier_slice_field_write_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_slice_field_write_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-bounded-carrier-slice-field-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bounded carrier slice field write canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("bounded carrier slice field write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `cells[0].label = \"Gate\"` to write the carrier through the slice pointer so \
         `cells[0].label == \"Gate\"` exits 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// #66 owned `[u8; N] in Utf8` carrier through HOST OUTPUT, native: build a carrier
// by concat and `write_line` it. The host-call path reads the carrier with carrier
// addressing (len @ 0, content pointer = place + pointer_size). Prints "Room A1"
// and exits 70.
#[test]
fn runtime_bounded_carrier_write_line_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_write_line_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-bounded-carrier-write-line-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bounded carrier write_line canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("bounded carrier write_line canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the carrier write_line canary to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end_matches(['\r', '\n']),
        "Room A1",
        "expected the carrier `write_line` to print the materialized content `Room A1`",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// #66 builder over NESTED-field carriers, CROSS-STATE: `self.line.text = "Room " +
// self.room.label` is built in `main` and `write_line`d in a later `shutdown`
// state. The nested fields carry their declared `in Utf8` domain across the state
// transition (entry-invariant seeded for nested fields, enforced at the nested
// write), so the carrier persists and prints. Prints "Room A1", exits 0.
#[test]
fn runtime_text_builder_canary_runs() {
    let canary = pass_canary("text/runtime_text_builder");
    let scratch =
        std::env::temp_dir().join(format!("omega-runtime-text-builder-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("nested-field carrier builder canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("nested-field carrier builder canary should run");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected the nested-field carrier builder canary to exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end_matches(['\r', '\n']),
        "Room A1",
        "expected the cross-state nested-field carrier builder to print `Room A1`",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// #66 (return a `&[u8] in Utf8` view from a machine): a value-position call
// returning a `&[u8] in Utf8` literal view flows as a real 16-byte `{ptr,len}`
// descriptor into a `==` content compare. `pick() == "Gate"` matches and exits 70;
// the interpreter agrees. Exercises the value-call-result descriptor reaching the
// TextEquals leaf.
#[test]
fn utf8_return_view_equals_exit_canary_runs() {
    let canary = pass_canary("domains/utf8_return_view_equals_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-utf8-return-view-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("utf8 return view canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("utf8 return view canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a returned `&[u8] in Utf8` view compared `== \"Gate\"` to match and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_shift_operators_exit_canary_runs() {
    let canary = pass_canary("operators/runtime_shift_operators_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-shift-operators-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("shift operators canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("shift operators canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `<<` and arithmetic `>>` (incl. negative) to evaluate correctly (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_bitwise_operators_exit_canary_runs() {
    let canary = pass_canary("operators/runtime_bitwise_operators_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-bitwise-operators-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bitwise operators canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("bitwise operators canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `&`, `|`, `^` to evaluate correctly (12&10==8, 12|10==14, 12^10==6; exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_popcount_loop_exit_canary_runs() {
    let canary = pass_canary("operators/runtime_popcount_loop_exit");
    let scratch = std::env::temp_dir().join(format!("omega-popcount-loop-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("popcount loop canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("popcount loop canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the shift-and-mask popcount loop to count 24 bits and self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_xorshift_prng_exit_canary_runs() {
    let canary = pass_canary("operators/runtime_xorshift_prng_exit");
    let scratch = std::env::temp_dir().join(format!("omega-xorshift-prng-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("xorshift prng canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("xorshift prng canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected xorshift32 (XOR + shifts composed) to draw 99 and self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_bitwise_guard_exit_canary_runs() {
    let canary = pass_canary("operators/runtime_bitwise_guard_exit");
    let scratch = std::env::temp_dir().join(format!("omega-bitwise-guard-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bitwise guard canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("bitwise guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected bitwise guard SUBJECTS (`flags & 2 == 0`, `& 4 == 4`, `| 2 == 7`, `^ 5 == 0`; exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn integer_literal_suffix_exit_canary_runs() {
    let canary = pass_canary("operators/integer_literal_suffix_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-integer-literal-suffix-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("integer literal suffix canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("integer literal suffix canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected suffixed integer literals (i64/u32/usize) to round-trip (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_value_position_branching_call_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_value_position_branching_call_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-value-position-branching-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("value-position branching call canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("value-position branching call canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `let r = self.classify(x)` to bind the selected arm value (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_free_machine_value_call_exit_canary_runs() {
    // A value-position call to a FREE stateful machine (top-level `machine pick`,
    // no attached data, 2-arm guarded value transition) must deliver the selected
    // arm's value. The backend state-call collector resolved only local states and
    // attached (method) machines, so `pick(self.v)` was never collected: `let n =
    // pick(self.v)` silently left n at 0 and a field target failed loudly. Covers
    // both a `let` local and a field target, both arms; exits 70 only when correct.
    let canary = pass_canary("calls/runtime_free_machine_value_call_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-free-machine-value-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("free-machine value call canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("free-machine value call canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `let n = pick(self.v)` / `self.low = pick(self.v)` on a free machine to bind the selected arm value (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_free_machine_struct_arg_exit_canary_runs() {
    // A BY-VALUE STRUCT argument to a FREE machine (`machine work(job: Job)`
    // called as `work(job)` / `combine(move pair)`) must deliver the caller's
    // field values. Three stacked selection bugs dropped the call's
    // result-slot write so the callee computed from a stale 0: the same-named
    // caller arg was rejected as a no-op self-binding (caller args arrive
    // symbol-less), caller-local initializer substitution had no Member arm
    // to project `job.id` through the struct literal, and the leaf terminal
    // value write resolved the substituted CALLER-context value in the
    // CALLEE's context. Rung 1 = same-name 1-field struct (71 on miss),
    // rung 2 = 2-field struct with explicit `move` (72). Exits 70 only when
    // both callees saw the real runtime field values.
    let canary = pass_canary("calls/runtime_free_machine_struct_arg_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-free-machine-struct-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("free-machine struct arg canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("free-machine struct arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected by-value struct args to free machines to deliver the caller's field values (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn by_value_case_param_self_write_exit_canary_runs() {
    // A `&mut self` machine taking a BY-VALUE CASE-BEARING parameter must
    // persist writes to `self.<field>` made in a dispatched substate.
    // Root cause: InlineBranching argument materialization had no handler for
    // StructLiteral arguments -- `Event::Insert { cents: 50 }` was never
    // written to the parameter slot, so the case tag stayed 0 (Idle), the
    // dispatch guard failed, the substate was never entered, and
    // `self.register.balance` stayed 0. Exits 70 when the write-back lands.
    let canary = pass_canary("calls/by_value_case_param_self_write_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-by-value-case-param-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("by-value case param self-write canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("by-value case param self-write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a by-value case-bearing arg to persist the self write-back (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_attached_machine_struct_arg_exit_canary_runs() {
    // The attached (data-scoped, receiverless `Worker::run`) spelling of the
    // by-value struct argument shape: the same leaf expansion path lowers it
    // (binding rewrite + struct-literal member projection + caller-context
    // value resolution), but resolution routes through the attached machine
    // lookup, so it gets its own rung. Exits 70 only when the callee saw the
    // real runtime field values (a dropped result-slot write reads 0 -> 71).
    let canary = pass_canary("calls/runtime_attached_machine_struct_arg_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-attached-machine-struct-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("attached-machine struct arg canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("attached-machine struct arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a by-value struct arg to an attached machine to deliver the caller's field values (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_record_forwarding_statement_call_exit_canary_runs() {
    // An inlined value call may execute ordinary statement callees before its
    // terminal value is delivered. The deferred leaf selector used to stop at
    // the outer callee's last direct mutation, so it copied `self.observed`
    // before the nested `capture()` mutation ran (native 0, interpreter 70).
    // The complete contiguous splice, including nested-callee operations, must
    // finish before the outer result slot is written. The same canary seeds an
    // omitted record field with 1 first, pinning whole-construction ZII reset
    // rather than merely the two explicitly named field writes.
    let canary = pass_canary("calls/runtime_record_forwarding_statement_call_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-record-forwarding-statement-call-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("record-forwarding statement-call canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("record-forwarding statement-call canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected nested statement effects to precede outer value delivery (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_free_machine_struct_return_exit_canary_runs() {
    // A FREE machine RETURNING a struct BY VALUE (`let lit: Pair = make(seed)`)
    // must deliver both field values into the caller's local. Two leaf
    // terminal-value resolution gaps dropped every per-field result-slot write
    // (the local read ZII zeroes): the caller-local initializer substitution
    // had no StructLiteral arm (folded caller locals never substituted inside
    // field values), and a local backed by a CALL's result slot (`let bumped =
    // bump(30)`) was substituted with the unloweable call expression instead
    // of keeping its name resolving against the result slot. Rung 1 = struct
    // from a folded literal seed, rung 2 = struct from a chained call-result
    // seed. Exits 70 only when all four returned fields are correct.
    let canary = pass_canary("calls/runtime_free_machine_struct_return_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-free-machine-struct-return-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("free-machine struct return canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("free-machine struct return canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected by-value struct returns from free machines to deliver both field values (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_free_machine_value_call_mut_arg_exit_canary_runs() {
    // A free-machine value call carrying a `&mut` tally argument alongside the
    // returned value: the callee increments the caller's field through the
    // reference AND returns the selected arm value. The tally is a counting probe
    // pinning call-count semantics (exactly one call). Exits 70 only when both
    // the returned value and the tally are correct.
    let canary = pass_canary("calls/runtime_free_machine_value_call_mut_arg_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-free-machine-mut-arg-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("free-machine mut-arg value call canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("free-machine mut-arg value call canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a free-machine value call with a &mut tally arg to return 100 and bump the tally once (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_free_machine_looping_value_call_exit_canary_runs() {
    // A value-position call to a LOOPING free machine (`count` walks a slice via
    // the self-recursive transition `count(s[1..], acc + 1)`). The recursive
    // target names the MACHINE, whose implicit body state is the generated
    // `entry` (attached machines name it after the method), so the transition
    // planner rejected it ("unknown state transition target"); it now resolves to
    // the entry segment as a real back-edge, and the looped accumulator is
    // delivered to the caller's `let n` slot. Exits 70 only when n == 5.
    let canary = pass_canary("calls/runtime_free_machine_looping_value_call_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-free-machine-looping-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("looping free-machine value call canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("looping free-machine value call canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a looping free-machine value call to deliver the looped accumulator (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_numeric_cast_exit_canary_runs() {
    let canary = pass_canary("expressions/runtime_numeric_cast_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-runtime-numeric-cast-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("numeric cast canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("numeric cast canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected numeric casts (float->int, int->float, signed widen) to evaluate correctly and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_widened_comparison_exit_canary_runs() {
    // The `as`-widen is the sanctioned way to compare different-width integers
    // (fail canary mismatched_width_comparison_rejected). Lock that the widened
    // compare does NOT truncate the wider operand: `44 as i32 == 300` is FALSE
    // (a truncating compare would read `44 == (300 & 0xFF == 44)` -> TRUE), while
    // `44 as i32 == 44` stays TRUE. Both correct -> exit 70.
    let canary = pass_canary("expressions/runtime_widened_comparison_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-runtime-widened-cmp-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("widened comparison canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("widened comparison canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the widened compare to avoid truncation (44 as i32 != 300, 44 as i32 == 44) and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_widened_bitwise_exit_canary_runs() {
    // Bitwise companion to runtime_widened_comparison_exit: `self.big | self.small
    // as u32` (u32 256 | widened u8 1) must be 257, not 1. A truncation to u8 width
    // (the rejected mismatched_width_bitwise bug) would drop the 256. -> exit 70.
    let canary = pass_canary("expressions/runtime_widened_bitwise_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-widened-bitor-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("widened bitwise canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("widened bitwise canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the widened bitwise OR to avoid truncation (256 | 1 == 257) and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_16bit_cast_exit_canary_runs() {
    let canary = pass_canary("expressions/runtime_16bit_cast_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-runtime-16bit-cast-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("16-bit cast canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("16-bit cast canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i16/u16 casts (truncate, sign/zero-extend, reinterpret) in every direction to evaluate correctly and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_float_place_comparison_exit_canary_runs() {
    let canary = pass_canary("expressions/runtime_float_place_comparison_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-float-place-compare-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("float place comparison canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("float place comparison canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected field-to-field float comparisons (<, >=, negative) to evaluate correctly and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_float_comparison_exit_canary_runs() {
    let canary = pass_canary("expressions/runtime_float_comparison_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-float-compare-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("float comparison canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("float comparison canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected float comparison guards (==, <, negative operand) to evaluate correctly and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_float_arithmetic_exit_canary_runs() {
    let canary = pass_canary("expressions/runtime_float_arithmetic_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-runtime-float-arith-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("float arithmetic canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("float arithmetic canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected float arithmetic (add/sub/mul/div + field operand) to execute and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_version_migration_exit_canary_runs() {
    // Historical and current shapes are ordinary data. The explicit migration
    // machine lands both current-shape writes and exits 70.
    let canary = pass_canary("versioning/runtime_version_migration_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-version-migration-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("version migration canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("version migration canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected CounterV1 construction + Counter::from_v1 migration to land both current-shape writes (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_versioned_match_zii_exit_canary_runs() {
    // An explicitly constructed ordinary lineage sum selects its V1 case even
    // though Current is written first; the historical payload drives exit 70.
    let canary = pass_canary("versioning/runtime_versioned_match_zii_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-versioned-match-zii-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("versioned match canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("versioned match canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the era-0 (v1) arm to be selected with a zero payload (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_versioned_three_era_match_zii_exit_canary_runs() {
    // An ordinary sum over three explicit era shapes selects V1 even though
    // both newer cases are written first; the V1 payload drives exit 70.
    let canary = pass_canary("versioning/runtime_versioned_three_era_match_zii_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-versioned-three-era-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("three-era versioned match canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("three-era versioned match canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the era-0 (v1) arm among three eras to be selected (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_equatable_scalar_not_equals_guard_exit_canary_runs() {
    // Equatable `!=` negation + `==` of a scalar record DIRECTLY in guard
    // position (the other equatable canaries route compares through a `let`).
    // String-bearing variants: pass/traits/equatable_string_not_equals_exit
    // (value position) and pass/traits/equatable_string_equality_guard_exit
    // (guard position).
    let canary = pass_canary("traits/runtime_equatable_scalar_not_equals_guard_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-equatable-scalar-neq-guard-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("equatable scalar != guard canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("equatable scalar != guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected !=/== in guard position over a scalar Equatable record to drive all three rungs (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_case_membership_mixed_shape_exit_canary_runs() {
    // Decision 11 `in` membership over a MIXED shape (decision 7): common
    // fields live between the tag and the payload overlay, so the membership
    // test must stay tag-only -- and survive a common-field write.
    let canary = pass_canary("data/runtime_case_membership_mixed_shape_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-membership-mixed-shape-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("mixed-shape membership canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("mixed-shape membership canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected tag-only membership over a mixed shape across all three rungs (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_roundtrip_repeated_max_one_exit_canary_runs() {
    // Wire exact-array field with the DEGENERATE extent `[u32; 1]`: one
    // required element, no synthetic count, and packed framing round-trips.
    let canary = pass_canary("wire/runtime_wire_roundtrip_repeated_max_one_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-wire-repeated-max-one-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("repeated max-one wire canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("repeated max-one wire canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the max=1 repeated field to round-trip (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_roundtrip_utf8_exit_canary_runs() {
    // &[u8]-in-Utf8 wire decode roundtrips for honest bytes (the
    // adversarial half is the pinned soundness hole).
    let canary = pass_canary("wire/runtime_wire_roundtrip_utf8_exit");
    let scratch = std::env::temp_dir().join(format!("omega-wireutf8-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("utf8 wire roundtrip canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("utf8 wire roundtrip canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "utf8 wire roundtrip canary should pass (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_utf8_edge_verdicts_exit_canary_runs() {
    // The utf8 validator's edge classes: honest multi-byte SOUND; overlong /
    // surrogate / beyond-max / truncated all INVALID.
    let canary = pass_canary("wire/runtime_wire_utf8_edge_verdicts_exit");
    let scratch = std::env::temp_dir().join(format!("omega-utf8edge-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("utf8 edge canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("utf8 edge canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "utf8 edge canary should agree on every class (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_utf8_invalid_refused_exit_canary_runs() {
    // Adversarial 0xFF 0xFF refuses with verdict Invalid on every engine.
    let canary = pass_canary("wire/runtime_wire_utf8_invalid_refused_exit");
    let scratch = std::env::temp_dir().join(format!("omega-utf8ref-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("utf8 refusal canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("utf8 refusal canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "adversarial bytes must refuse (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_schema_as_value_type_exit_canary_runs() {
    // A numbered data serves as a plain program type + encodes from itself.
    let canary = pass_canary("wire/runtime_wire_schema_as_value_type_exit");
    let scratch = std::env::temp_dir().join(format!("omega-schemaval-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("schema-as-value canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("schema-as-value canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "schema-as-value canary should pass (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_decode_let_compare_exit_canary_runs() {
    // A let-bound comparison of a decoded field reads the DECODED value
    // (the wire selection clears the static-value table).
    let canary = pass_canary("wire/runtime_wire_decode_let_compare_exit");
    let scratch = std::env::temp_dir().join(format!("omega-declc-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("decode-let-compare canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("decode-let-compare canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "decode-let-compare canary should read decoded values (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_encode_repeated_then_string_exit_canary_runs() {
    // Wire repeated + String-last in one message: two runtime-sized appends
    // in sequence -- the String's cursor must start where the packed payload
    // actually ended. Encode-only (String decode has not landed); the exact
    // 10 bytes are asserted in-program.
    let canary = pass_canary("wire/runtime_wire_encode_repeated_then_string_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-wire-repeated-string-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("repeated-then-string wire canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("repeated-then-string wire canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the packed repeated payload then the String field to encode the asserted 10 bytes (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_roundtrip_nested_and_repeated_exit_canary_runs() {
    // Wire nested message + repeated field in ONE message: both stage
    // runtime-sized payloads, so the composition pins cursor handoff between
    // them on both the encode and decode sides (written = read = 13).
    let canary = pass_canary("wire/runtime_wire_roundtrip_nested_and_repeated_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-wire-nested-repeated-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("nested-and-repeated wire canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("nested-and-repeated wire canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the nested sub-message and the packed repeated field to round-trip together (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_const_array_length_transitive_exit_canary_runs() {
    // Comptime stage 1, transitive admission: the const-position callee CALLS
    // another build-time-admissible machine (base() * 3 + 1 = 16), pinning
    // that const evaluation runs the call machinery, not just expression folding.
    let canary = pass_canary("comptime/runtime_const_array_length_transitive_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-const-length-transitive-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("transitive const length canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("transitive const length canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the transitively-evaluated 16-slot array to hold both end values (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_const_array_length_bare_call_arm_exit_canary_runs() {
    // Comptime: the const-position callee's value arm is a PARENTHESIZED
    // BARE CALL (`_ -> (burn(4, 12))`). The parenthesized lone call is a
    // value expression (not a transition target), so const evaluation
    // resolves the free machine `burn` like the arithmetic-wrapped spelling
    // does: 16 slots, both the write and the index-15 typecheck land.
    let canary = pass_canary("comptime/runtime_const_array_length_bare_call_arm_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-const-length-bare-call-arm-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bare-call-arm const length canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("bare-call-arm const length canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the bare-call-arm const length (burn(4, 12) = 16) to size the array (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}
