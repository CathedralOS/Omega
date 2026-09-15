use super::fixture_roster;
use crate::{Command, compile_rooted_canary_for_native_host, executable_name, fs, pass_canary};

#[test]
fn runtime_computed_array_fill_via_temp_exit_canary_runs() {
    // The sound pattern for filling an array with computed values in a write-first loop: a computed
    // value goes to a field, then the field (a machine-resident source) is copied to the runtime-
    // indexed element -- native emission cannot yet store a computed expression straight into an
    // indexed element. Fills [0,10,20,30,40], sums to 100, self-checks -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_COMPUTED_ARRAY_FILL_VIA_TEMP_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-computed-fill-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("computed array-fill canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("computed array-fill canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("computed array-fill canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a computed value written via a field temp then indexed-copied to fill correctly (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_loop_fill_exit_canary_runs() {
    // Nested loops: an outer loop drives an inner write-first loop that fills a row (counter
    // reset each outer pass). Exercises the loop-invariant machinery in a nested context -- the
    // inner head sits inside the outer loop's natural loop yet its own back-edge guard still
    // proves the write. Sum self-checks to 3 -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_LOOP_FILL_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-nested-loop-fill-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested-loop fill canary should compile from its authored root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("nested-loop fill canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected an inner write-first loop nested in an outer loop to prove its bound and fill correctly (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_loop_counter_init_hoisted_exit_canary_runs() {
    // The loop counter is initialized one state BEFORE the loop head (a `setup` state that does
    // not touch the counter sits between). The loop-invariant pass walks back through the
    // counter-untouched state to find the constant init, so the fill loop's index bound proves.
    // Fills [0..4], sums to 10, self-checks -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_LOOP_COUNTER_INIT_HOISTED_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-init-hoisted-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("init-hoisted loop canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("init-hoisted loop canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("init-hoisted loop canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a loop counter initialized a state before the head to still prove its bound (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_write_first_loop_index_exit_canary_runs() {
    // Write-first loop `arr[i]=..; i=i+1; transition i<N { true -> loop }`: the bound guard is on
    // the back edge, so the head is a join with no dominating guard. The loop-invariant pass now
    // carries `i < N` (from the back-edge guard) at the head's entry for a monotone-increasing
    // counter, so the write proves. Fills [0..4], sums to 10, self-checks -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_WRITE_FIRST_LOOP_INDEX_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-write-first-loop-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("write-first loop canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("write-first loop canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("write-first loop canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a write-first increasing loop to prove its index bound and fill correctly (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_array_indexed_loop_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_ARRAY_INDEXED_LOOP_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-array-indexed-loop-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime array indexed loop canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime array indexed loop canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("runtime array indexed loop canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a runtime-indexed loop to sum the array to 100 and self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_decreasing_index_exit_canary_runs() {
    // A DECREASING runtime counter indexing an inline array: the bound
    // `self.i < 4` that the body's `self.nums[self.i]` needs is a loop
    // INVARIANT (entry `i = 3`, each `i = i - 1` decrement preserves `i < 4`),
    // not the loop guard (`self.i >= 0`). The loop head is multi-predecessor, so
    // single-predecessor incoming-guard seeding can't reach it; the inductive
    // loop-invariant fact discharges the index obligation. Sums [1,2,3,4]
    // backwards to 10 and self-checks (exit 70).
    let canary = pass_canary(fixture_roster::RUNTIME_DECREASING_INDEX_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-decreasing-index-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime decreasing index canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime decreasing index canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("runtime decreasing index canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a decreasing-counter loop (loop-invariant bound) to sum [1,2,3,4] \
         backwards to 10 and self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_indexed_read_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_INDEXED_READ_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-indexed-read-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime slice indexed read canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime slice indexed read canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("runtime slice indexed read canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `s[self.i]` (runtime index on a &[T] slice) to read 20 and 40 and self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_array_adjacent_index_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_ARRAY_ADJACENT_INDEX_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-adjacent-index-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime adjacent-index canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime adjacent-index canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("runtime adjacent-index canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the DERIVED index `nums[j + 1]` (bound carried across `jp = j + 1`) to walk adjacent pairs and confirm the array is sorted (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_decreasing_index_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_DECREASING_INDEX_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-nested-decreasing-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime nested-decreasing-index canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime nested-decreasing-index canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected NESTED decreasing loops -- the inner counter's invariant proven via dominance-based back edges, the outer invariant held through the inner loop -- to sum to 54 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_narrow_widen_cast_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_NARROW_WIDEN_CAST_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-narrow-widen-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime narrow-widen-cast canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("narrow-widen cast canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime narrow-widen-cast canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected inline named conversion + policy qualification to consume the delivered call result and extend by signedness -- u8>127 zero-extends (sum 806), i8<0 sign-extends (-5) (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_signed_index_guarded_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SIGNED_INDEX_GUARDED_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-signed-index-guarded-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime signed-index-guarded canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime signed-index-guarded canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("runtime signed-index-guarded canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a SIGNED i32 index proven non-negative by its `>= 0` guard to be accepted and sum nums[3..0] to 10 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_two_pointer_sum_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_TWO_POINTER_SUM_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-two-pointer-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime two-pointer-sum canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime two-pointer-sum canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("runtime two-pointer-sum canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the two-pointer traversal to prove nums[i] via the relational chain (i <= j < len) and sum converging pairs to 210 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_two_pointer_reverse_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_TWO_POINTER_REVERSE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-two-pointer-reverse-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime two-pointer-reverse canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime two-pointer-reverse canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("runtime two-pointer-reverse canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected two-pointer in-place reverse (indexed WRITE targets proved via the relational chain) to reverse [1..5] to [5..1] (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_branched_index_bound_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_BRANCHED_INDEX_BOUND_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-branched-bound-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime branched-index-bound canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime branched-index-bound canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("runtime branched-index-bound canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a loop bound to carry TRANSITIVELY across a conditional branch so the indexed read in the branch target proves, re-reading 99 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_array_write_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_ARRAY_WRITE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-indexed-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime indexed-array-write canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime indexed-array-write canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("runtime indexed-array-write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a runtime-indexed array WRITE of a field value (`nums[self.i] = self.v`) to fill nums[i]=i+100 and read 103 back at index 3 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn recursive_subslice_element_accumulator_exit_canary_runs() {
    // `sum(s[1..], acc + s[0])`: the element read s[0] must happen before the
    // s descriptor is retargeted to s[1..]. Was an off-by-one (descriptor
    // advanced first -> summed the next window's head -> native exit 71).
    let canary = pass_canary(fixture_roster::RECURSIVE_SUBSLICE_ELEMENT_ACCUMULATOR_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-recursive-subslice-accum-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("recursive subslice element accumulator canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("recursive subslice element accumulator canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected sum([5,10,15,20]) == 50 via sum(s[1..], acc + s[0]) and exit 70, got {:?} (71 = descriptor advanced before s[0] read)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_of_slice_param_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SUBSLICE_OF_SLICE_PARAM_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-param-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime subslice of slice param canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime subslice of slice param canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("runtime subslice of slice param canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected subslicing a runtime slice param to shrink the length (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_field_subslice_arg_index_exit_canary_runs() {
    // Passing a BARE subslice of a machine fixed-array field (`self.source[0..3]`,
    // no `.as_slice()`) as a `&[u8]` argument must materialize a correct
    // {ptr,len} descriptor. The literal-subslice descriptor writer only knew
    // `x.as_slice()[a..b]` bases, so a bare base declined and the argument fell
    // through to a garbage copy (wrong len AND elements natively). Exits 70.
    let canary = pass_canary(fixture_roster::RUNTIME_MACHINE_FIELD_SUBSLICE_ARG_INDEX_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-machine-field-subslice-arg-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("machine-field subslice arg canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("machine-field subslice arg canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("machine-field subslice arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a bare machine-field subslice passed as a slice arg to carry a correct descriptor (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_index_read_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_INDEX_READ_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-slice-read-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime slice index read canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("slice index read canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime slice index read canary should run");

    assert_eq!(
        output.status.code(),
        Some(41),
        "expected runtime slice index read canary to preserve dynamic slice reads and exit 41, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_read_operand_exit_canary_runs() {
    // A runtime-indexed read `self.nums[self.i]` used as a SUB-EXPRESSION OPERAND
    // (a child of `+` and of the ordinary `widen_i32_to_i64` conversion),
    // hoisted into synthetic `let __hoist_N = self.nums[self.i];` temps.
    // Exits 70 when acc == 20 and big == 20.
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_READ_OPERAND_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-indexed-operand-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime indexed read operand canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed read operand canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime indexed read operand canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected hoisted runtime-indexed operand reads (binary + cast) to lower and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}
