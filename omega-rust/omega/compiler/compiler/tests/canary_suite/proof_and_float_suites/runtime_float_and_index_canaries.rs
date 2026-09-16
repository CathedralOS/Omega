use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, Command, CompileRequest, CompilerOptions,
    RequestedCompileProduct, compile, compile_reviewed_repository_fixture,
    compile_rooted_canary_for_native_host, executable_name, fs, interpret, pass_canary,
};
use compiler::CheckedCompileRequest;

#[test]
fn runtime_float_min_max_abs_clamp_exit_canary_runs() {
    // Float min/max on SSE (maxsd/minsd), plus abs/clamp over floats which
    // desugar to them: max(3,7)+min(3,7)+abs(-12)+clamp(300,0,200) = 222.
    let canary = pass_canary(fixture_roster::ARITHMETIC_RUNTIME_FLOAT_MIN_MAX_ABS_CLAMP_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-float-minmax-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("float min/max/abs/clamp canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float min/max/abs/clamp canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected float max/min/abs/clamp to sum to 222 (exit 70); exit 71 = maxsd/minsd \
         disagreed with the interpreter. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_shared_ref_param_member_exit_canary_runs() {
    // Shared &Struct param (non-boundary): content-spill convention; the callee
    // reads a=7 and b=35 through the ref -> got=42 (the exit). A pointee
    // misresolution dereferences the spilled content and crashes (0xC0000005).
    let canary = pass_canary(fixture_roster::CALLS_RUNTIME_SHARED_REF_PARAM_MEMBER_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-sharedref-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("shared-ref param canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("shared-ref param canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("shared-ref param canary should run");

    assert_eq!(
        output.status.code(),
        Some(42),
        "expected a+b=42 through the shared &Struct param (exit 42); a crash/garbage =          the pointee resolver treated the content spill as a pointer. got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_shared_ref_param_large_deref_exit_canary_runs() {
    // The DEREF twin of the content-spill canary above (bug 2026-07-12): a
    // shared &Struct param whose referee is LARGER than a pointer holds a real
    // pointer (it cannot be content-spilled into the 8-byte slot), so a
    // local-init member read (`let v = r.value`, Cathedral's `let bs =
    // table.boot_services` shape) MUST dereference. Reading the slot inline
    // instead fetched garbage -- Cathedral's M2 boot dispatched get_memory_map
    // through it and #UD'd under QEMU. value=42 at offset 16 -> exit 42.
    let canary = pass_canary(fixture_roster::CALLS_RUNTIME_SHARED_REF_PARAM_LARGE_DEREF_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-sharedref-large-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("large shared-ref deref canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("large shared-ref deref canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("large shared-ref deref canary should run");

    assert_eq!(
        output.status.code(),
        Some(42),
        "expected value=42 dereferenced through the large shared &Struct param (exit 42); \
         an inline (non-deref) read of the pointer-sized slot fetches garbage. got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_large_shared_ref_direct_assignment_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::CALLS_RUNTIME_LARGE_SHARED_REF_DIRECT_ASSIGNMENT_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-large-shared-ref-direct-assignment-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("large shared-ref direct-assignment canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("large shared-ref direct-assignment canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("large shared-ref direct-assignment canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "large shared-ref argument/assignment lost address identity; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_same_type_contained_direct_fields_exit_canary_runs() {
    // The SOUND pattern for two same-type contained machines: DIRECT field access
    // (not method calls, which alias to the first field of the type). a -> 13,
    // b -> 21 independently -> exit 70.
    let canary = pass_canary(fixture_roster::CALLS_RUNTIME_SAME_TYPE_CONTAINED_DIRECT_FIELDS_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-sametype-direct-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("same-type contained direct-fields canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("same-type direct-fields canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("same-type contained direct-fields canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected two same-type contained machines to be INDEPENDENT via direct field \
         access (a=13, b=21 -> exit 70); exit 71 = they aliased. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_sum_field_store_payload_exit_canary_runs() {
    // Regression for the sum-type field-store payload-offset miscompile: storing
    // Tx::Transfer{to:3, amount:40} into a field then matching must read to=3,
    // amount=40 -> exit 70 (before the fix: to=40, amount=0).
    let canary = pass_canary(fixture_roster::CONTROL_FLOW_RUNTIME_SUM_FIELD_STORE_PAYLOAD_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-sumfield-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("sum-field-store payload canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("sum-field-store payload canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("sum-field-store payload canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected field-stored Tx::Transfer to read to=3, amount=40 (exit 70); exit 71 = the \
         payload offset shifted (write not variant-tagged). got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_argmax_index_exit_canary_runs() {
    // argmax over [4,15,8,42,16,23]: the maximum 42 is at index 3 -> exit 70;
    // a wrong index-capture -> exit 71.
    let canary = pass_canary(fixture_roster::COLLECTIONS_RUNTIME_ARGMAX_INDEX_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-argmax-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("argmax canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("argmax canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("argmax canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected argmax of [4,15,8,42,16,23] to be index 3 (exit 70); exit 71 = wrong \
         index capture. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_bracket_matcher_stack_exit_canary_runs() {
    // Stack-based bracket matcher over "([)]" (mis-nested). Correct verdict is
    // UNBALANCED -> exit 70; a count-only or broken matcher -> exit 71.
    let canary = pass_canary(fixture_roster::COLLECTIONS_RUNTIME_BRACKET_MATCHER_STACK_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-bracket-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("bracket matcher canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("bracket matcher canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("bracket matcher canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the stack matcher to detect mis-nesting in \"([)]\" (exit 70); exit 71 = \
         it accepted the mismatch. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_palindrome_two_pointer_exit_canary_runs() {
    // Two-pointer palindrome over [1,2,3,4,1] (NOT a palindrome): must detect the
    // arr[1]=2 vs arr[3]=4 mismatch -> exit 70; missing it -> exit 71.
    let canary = pass_canary(fixture_roster::COLLECTIONS_RUNTIME_PALINDROME_TWO_POINTER_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-palindrome-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("palindrome two-pointer canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("palindrome two-pointer canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("palindrome two-pointer canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the two-pointer scan to DETECT arr[1]=2 != arr[3]=4 (exit 70); exit 71 = \
         it missed the mismatch. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_cross_array_indexed_guard_compare_exit_canary_runs() {
    // PROBE: `a[i] < b[j]` (two different arrays, both runtime indices) in a
    // guard. a[1]=20 < b[3]=4 is FALSE; reverse TRUE -> exit 70.
    let canary =
        pass_canary(fixture_roster::COLLECTIONS_RUNTIME_CROSS_ARRAY_INDEXED_GUARD_COMPARE_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-cross-idx-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("cross-array indexed guard-compare canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("cross-array indexed guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("cross-array indexed guard-compare canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a[1]=20 < b[3]=4 FALSE and reverse TRUE (exit 70); exit 71 = base/index \
         confusion across the two arrays. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dual_indexed_guard_equality_exit_canary_runs() {
    // PROBE: `arr[i] == arr[j]` (equality op, both runtime indices) in a guard.
    // arr[0]=10 == arr[3]=10 TRUE; arr[0]=10 == arr[1]=20 FALSE -> exit 70.
    let canary = pass_canary(fixture_roster::COLLECTIONS_RUNTIME_DUAL_INDEXED_GUARD_EQUALITY_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-dual-idx-eq-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dual-indexed guard-equality canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dual-indexed guard-equality canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dual-indexed guard-equality canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected arr[0]==arr[3] TRUE and arr[0]==arr[1] FALSE (exit 70); exit 71 = the \
         equality compared the wrong elements. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dual_indexed_guard_compare_exit_canary_runs() {
    // PROBE: `arr[i] < arr[j]` (both runtime indices) in a guard. arr[1]=20 <
    // arr[3]=40 is TRUE -> exit 70. Exit 71 = silent miscompile.
    let canary = pass_canary(fixture_roster::COLLECTIONS_RUNTIME_DUAL_INDEXED_GUARD_COMPARE_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-dual-idx-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dual-indexed guard-compare canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dual-indexed guard-compare canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dual-indexed guard-compare canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected arr[1]=20 < arr[3]=40 to be TRUE (exit 70); exit 71 = the guard \
         compared the wrong elements (silent miscompile). got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_float_running_min_max_fold_exit_canary_runs() {
    // A RUNNING float min/max fold across state edges: `self.lo = min(self.lo,
    // self.cur)` reads an accumulator field, feeds minsd/maxsd, and writes it
    // back each iteration (the constant-operand canary never reaches this
    // field-read-then-write-back path). Over [5, 2, 8, 3]: lo->2, hi->8, sum 10.
    let canary = pass_canary(fixture_roster::ARITHMETIC_RUNTIME_FLOAT_RUNNING_MIN_MAX_FOLD_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-float-minmax-fold-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("float running min/max fold canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float running min/max fold canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected running float min/max fold over [5,2,8,3] to give lo=2,hi=8,sum=10 \
         (exit 70); exit 71 = the field-accumulator min/max fold disagreed. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_clamp_desugar_exit_canary_runs() {
    // `clamp(x, lo, hi)` = `min(max(x, lo), hi)`: 300->255, -5->0, 128->128.
    let canary = pass_canary(fixture_roster::ARITHMETIC_RUNTIME_CLAMP_DESUGAR_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-clamp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("clamp desugar canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("clamp desugar canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected clamp above/below/within to give 255/0/128 (exit 70); got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_clamp_narrowing_exit_canary_runs() {
    // clamp's [0,100] result interval flows to the decision-17 narrowing check,
    // so `self.i8 = clamp(self.i32, 0, 100)` compiles AND the backend stores the
    // clamped value: clamp(300,0,100)=100 lands in i8, read back = exit 100.
    let canary = pass_canary(fixture_roster::ARITHMETIC_RUNTIME_CLAMP_NARROWING_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-clamp-narrow-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("clamp narrowing canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("clamp narrowing canary should run");

    assert_eq!(
        output.status.code(),
        Some(100),
        "expected clamp(300,0,100)=100 stored in i8 (exit 100); got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_negative_float_to_int_exit_canary_runs() {
    // Negative f64 -> i32 truncates toward zero: -3.7 -> -3, -100.0 -> -100.
    // Guards on the results and exits 70 (a positive code) so the assertion is
    // robust to shells that mangle negative process exit codes.
    let canary = pass_canary(fixture_roster::ARITHMETIC_RUNTIME_NEGATIVE_FLOAT_TO_INT_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-negfloat-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("negative float->int canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("negative float->int canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected -3.7->-3 and -100.0->-100 (exit 70); got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn float_to_int_policy_canary_runs() {
    let canary = pass_canary(fixture_roster::FLOAT_FLOAT_TO_INT_POLICY_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-float-int-policy-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("float-to-int policy canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float-to-int policy canary should run");
    assert_eq!(
        output.status.code(),
        Some(77),
        "expected target-width saturation and NaN->0 (exit 77); got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn float_to_int_exact_proofs_canary_runs() {
    let canary = pass_canary(fixture_roster::FLOAT_FLOAT_TO_INT_EXACT_PROOFS_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-float-int-exact-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("proven Exact float-to-int canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("proven Exact float-to-int canary should run");
    assert_eq!(output.status.code(), Some(78));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn float_to_int_trapping_canaries_abort() {
    for &name in fixture_roster::FLOAT_TO_INT_TRAPPING_PASS_CANARIES {
        let canary = pass_canary(name);
        let leaf = name.rsplit('/').next().unwrap_or("trap");
        let build_dir =
            std::env::temp_dir().join(format!("omega-float-int-{leaf}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&build_dir);
        compile_rooted_canary_for_native_host(&canary, build_dir.clone())
            .expect("Trapping float-to-int canary should compile");
        let output = Command::new(build_dir.join(executable_name()))
            .output()
            .expect("Trapping float-to-int canary should start");
        assert!(
            !output.status.success(),
            "{name} reached exit(70) instead of trapping"
        );
        assert_ne!(
            output.status.code(),
            Some(70),
            "{name} reached its post-operation sentinel instead of trapping"
        );
        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn float_saturating_arithmetic_canary_runs() {
    let canary = pass_canary(fixture_roster::FLOAT_FLOAT_SATURATING_ARITHMETIC_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-float-saturating-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("Saturating float arithmetic canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("Saturating float arithmetic canary should run");
    assert_eq!(
        output.status.code(),
        Some(77),
        "expected overflow-only f32 saturation (exit 77); got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn float_trapping_arithmetic_canaries_abort() {
    for &name in fixture_roster::FLOAT_TRAPPING_ARITHMETIC_PASS_CANARIES {
        let canary = pass_canary(name);
        let leaf = name.rsplit('/').next().unwrap_or("trap");
        let build_dir =
            std::env::temp_dir().join(format!("omega-float-{leaf}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&build_dir);
        compile_rooted_canary_for_native_host(&canary, build_dir.clone())
            .expect("Trapping float arithmetic canary should compile from its authored root");
        let output = Command::new(build_dir.join(executable_name()))
            .output()
            .expect("Trapping float arithmetic canary should start");
        assert!(
            !output.status.success(),
            "{name} reached exit(70) instead of trapping"
        );
        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn runtime_sqrt_builtin_exit_canary_runs() {
    // `sqrt(x)` unary float builtin: f64 sqrt(64)=8, f32 sqrt(9)=3, via the
    // native sqrtsd/sqrtss path (both operands = x on the binary SSE lane).
    let canary = pass_canary(fixture_roster::ARITHMETIC_RUNTIME_SQRT_BUILTIN_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-sqrt-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("sqrt builtin canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("sqrt builtin canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected sqrt(64.0)=8.0 and sqrt(9.0f32)=3.0 (exit 70); got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_abs_desugar_exit_canary_runs() {
    // `abs(x)` desugars to `max(x, 0 - x)` (frontend-only; min/max are binary
    // builtins). abs(-70)=70 and abs(12)=12; exit 70 confirms both.
    let canary = pass_canary(fixture_roster::ARITHMETIC_RUNTIME_ABS_DESUGAR_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-abs-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("abs desugar canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("abs desugar canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected abs(-70)=70 and abs(12)=12 (exit 70); got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_float_self_compare_nan_exit_canary_runs() {
    // The canonical isNaN idiom `f != f` (and its `f == f` complement) on a
    // NaN operand: TRUE/FALSE per IEEE. Was silently folded to constants by
    // the untyped reflexive fold; the fold is TYPE-GATED now and float
    // self-compares lower to the real ucomis* runtime compare.
    let canary = pass_canary(fixture_roster::ARITHMETIC_RUNTIME_FLOAT_SELF_COMPARE_NAN_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-nan-self-compare-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("NaN self-compare canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("NaN self-compare canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `f != f` TRUE and `f == f` FALSE for NaN (exit 70); exit 71 = a reflexive \
         fold collapsed the float self-compare to a constant. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_total_order_satisfiers_exit_canary_runs() {
    // F6: the explicit f32/f64 total orders are named trait satisfiers, and a
    // generic consumer selects their concrete machine symbols statically. Raw
    // NaN payloads and signed zero distinguish this from arithmetic `<`.
    let canary = pass_canary(fixture_roster::FLOAT_RUNTIME_TOTAL_ORDER_SATISFIERS_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("total-order satisfier canary should compile to checked trees");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(
        interpreted.error, None,
        "interpreter should support the total-order satisfier path"
    );
    assert_eq!(
        interpreted.exit_code, 70,
        "interpreter should preserve the complete f32/f64 total-order edge set"
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-total-float-order-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("total-order satisfier canary should compile natively");

    let executable = compilation
        .checked_native_executable_path()
        .expect("total-order satisfier canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("total-order satisfier canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected IEEE totalOrder over NaNs/infinities/signed zero (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    // Keep both native instruction-selection families on the same static
    // satisfier path even when the host can execute only one of them.
    for target in ["linux_x86_64", "linux_arm64"] {
        let cross_dir = std::env::temp_dir().join(format!(
            "omega-total-float-order-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&cross_dir);
        compiler::compile(
            CompileRequest::new(CompilerOptions {
                root_path: main_path.clone(),
                build_dir: Some(cross_dir.clone()),
                target_name: Some(target.into()),
            })
            .with_requested_product(RequestedCompileProduct::NativeArtifact),
        )
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|error| {
            panic!("total-order satisfier canary should cross-compile for {target}: {error:?}")
        });
        let _ = fs::remove_dir_all(&cross_dir);
    }
}
