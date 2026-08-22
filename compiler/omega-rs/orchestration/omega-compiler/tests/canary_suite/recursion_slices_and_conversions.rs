use super::*;

#[test]
fn runtime_nat_structural_recursion_exit_canary_runs() {
    // N2(d) gateway: a free machine over proof-only Nat is a PROOF MACHINE
    // -- structural recursion legal, measured, every self-call descending
    // by a case-payload subterm; the program lowers without it.
    let canary = pass_canary("proofs/runtime_nat_structural_recursion_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-nat-structural-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nat structural recursion canary should compile from its authored root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("nat structural recursion canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the proof machine to validate and the program to run (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_core_nat_declared_exit_canary_runs() {
    // N4 first slice: core Nat loads through the bundled root; declaring
    // proof-only data never touches runtime.
    let canary = pass_canary("proofs/runtime_core_nat_declared_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-core-nat-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("core Nat canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("core Nat canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("core Nat canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the core-Nat-using program to run untouched (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn accepted_axiom_cited_exit_canary_runs() {
    // CH10 GR6d: a bodyless boundary machine (accepted axiom) parses, its
    // ensures is believed under dev-active grant locality, and a lemma
    // citing it proves through the accepted fact. Runs untouched.
    let canary = pass_canary("proofs/accepted_axiom_cited_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-accepted-axiom-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host_with_auxiliary_artifacts(&canary, build_dir.clone())
        .expect("accepted-axiom canary should compile from its authored root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("accepted-axiom canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the axiom-citing program to run untouched (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let report = fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    assert!(
        report.contains("accepted fact: mul_comm_axiom"),
        "the axiom must surface as a trust row:\n{report}"
    );
    assert!(
        report.contains("STANDING WARNING"),
        "an ungranted axiom is dev-active with the standing warning:\n{report}"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_core_rat_declared_exit_canary_runs() {
    // N4 Rat rung: the canonical-representative Rat carrier loads through
    // the bundled core; proof-only data over Nat never touches runtime.
    let canary = pass_canary("proofs/runtime_core_rat_declared_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-core-rat-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("core Rat canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("core Rat canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("core Rat canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the core-Rat-using program to run untouched (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_free_const_exit_canary_runs() {
    // M2 blocker 4: free-floating consts substitute behind the shadowing
    // walk.
    let canary = pass_canary("constants/runtime_free_const_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-free-const-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("free const canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("free const canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("free const canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the bare PAGE_SIZE to substitute 64 (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_value_call_terminal_exit_canary_runs() {
    // Free value-machine calls in TERMINAL position (always-arm transition
    // values and trailing returns) hoist into the let-bound spelling:
    // single-state callee (72 on miss), multi-state acyclic (73), and the
    // cyclic cos-via-terminal shape that used to overflow the compile
    // thread (74).
    let canary = pass_canary("calls/runtime_value_call_terminal_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-terminal-call-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("value-call terminal canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("value-call terminal canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected all three hoisted terminal-call shapes to deliver (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_result_domain_machine_overload_exit_canary_runs() {
    let canary = pass_canary("domains/runtime_result_domain_machine_overload_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-result-domain-overload-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("result-domain machine overload canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("result-domain overload canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("result-domain machine overload canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected qualified and empty result sets to select distinct callees, got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_std_math_sin_cos_exit_canary_runs() {
    // std math natively: sin's polynomial (exit 72 on miss), the binary
    // ladder at sin(10) (73), and the let-bound cos composition (74) --
    // delivered through the FLOAT binary terminal return-write.
    let canary = pass_canary("calls/runtime_std_math_sin_cos_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-std-sin-cos-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("std math sin/cos canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("std math sin/cos canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected sin(1)/sin(10)/cos(1) inside their 1e-11 windows (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_computed_index_match_subject_exit_canary_runs() {
    // R0's last position: a computed index in an enum-match SUBJECT hoists
    // like every other position.
    let canary = pass_canary("collections/runtime_computed_index_match_subject_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-match-subject-index-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("computed-index match subject canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("computed-index match subject canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("computed-index match subject canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected grid[1*3+2] to classify as Goal (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_const_measured_recursion_exit_canary_runs() {
    // MR5: measured tail recursion evaluates at compile time under the
    // const-eval fuel cap to size a fixed array.
    let canary = pass_canary("comptime/runtime_const_measured_recursion_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-const-recursion-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("const measured recursion canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("const measured recursion canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected triangle(4)=10 to size the buffer (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_terminal_tail_recursion_exit_canary_runs() {
    // MR2 complete: the terminal tail call rewrites onto the loop-back and
    // the fall-through complement proves the decrease.
    let canary = pass_canary("calls/runtime_terminal_tail_recursion_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-terminal-tail-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("terminal tail recursion canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("terminal tail recursion canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the terminal tail loop to reach the base case (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_measured_tail_recursion_exit_canary_runs() {
    // MR1: the call-spelled tail arm on a measured machine resolves onto
    // the bare loop-back edge and runs.
    let canary = pass_canary("calls/runtime_measured_tail_recursion_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-measured-tail-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("measured tail recursion canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("measured tail recursion canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the measured tail loop to count down to 7 (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_u64_guarded_cap_store_exit_canary_runs() {
    // N2 rung (c): the guarded-copy discharge survives the exact u64 range
    // fact (the retired i64::MAX cap's positive twin).
    let canary = pass_canary("arithmetic/runtime_u64_guarded_cap_store_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-u64-guarded-cap-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("guarded cap-store canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("guarded cap-store canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("guarded cap-store canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the guarded u64 copy to store and exit 70, got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_proof_only_data_declared_exit_canary_runs() {
    // Math roster N1: declaring recursive (proof-only) data is legal; the
    // classification fences consumption, not declaration.
    let canary = pass_canary("data/runtime_proof_only_data_declared_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-proof-only-declared-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("proof-only declaration canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("proof-only declaration canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("proof-only declaration canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected declared-but-unconsumed proof-only data to leave runtime untouched (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_f32_field_guard_exit_canary_runs() {
    // Plain f32 field guards: f32-pattern expectations, 4-byte compares.
    let canary = pass_canary("arithmetic/runtime_f32_field_guard_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-f32-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("f32 field-guard canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("f32 field-guard canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the f32 field guard to compare at f32 width (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_computed_array_fill_via_temp_exit_canary_runs() {
    // The sound pattern for filling an array with computed values in a write-first loop: a computed
    // value goes to a field, then the field (a machine-resident source) is copied to the runtime-
    // indexed element -- native emission cannot yet store a computed expression straight into an
    // indexed element. Fills [0,10,20,30,40], sums to 100, self-checks -> exit 70.
    let canary = pass_canary("collections/runtime_computed_array_fill_via_temp_exit");
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
    let canary = pass_canary("collections/runtime_nested_loop_fill_exit");
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
    let canary = pass_canary("collections/runtime_loop_counter_init_hoisted_exit");
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
    let canary = pass_canary("collections/runtime_write_first_loop_index_exit");
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
    let canary = pass_canary("slices/runtime_array_indexed_loop_exit");
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
    let canary = pass_canary("slices/runtime_decreasing_index_exit");
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
    let canary = pass_canary("slices/runtime_slice_indexed_read_exit");
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
    let canary = pass_canary("slices/runtime_array_adjacent_index_exit");
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
    let canary = pass_canary("slices/runtime_nested_decreasing_index_exit");
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
    let canary = pass_canary("slices/runtime_narrow_widen_cast_exit");
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
    let canary = pass_canary("slices/runtime_signed_index_guarded_exit");
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
    let canary = pass_canary("slices/runtime_two_pointer_sum_exit");
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
    let canary = pass_canary("slices/runtime_two_pointer_reverse_exit");
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
    let canary = pass_canary("slices/runtime_branched_index_bound_exit");
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
    let canary = pass_canary("slices/runtime_indexed_array_write_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-indexed-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime indexed-array-write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
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
    let canary = pass_canary("slices/recursive_subslice_element_accumulator_exit");
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
    let canary = pass_canary("slices/runtime_subslice_of_slice_param_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-param-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime subslice of slice param canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
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
    let canary = pass_canary("slices/runtime_machine_field_subslice_arg_index_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-machine-field-subslice-arg-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("machine-field subslice arg canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
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
    let canary = pass_canary("slices/runtime_slice_index_read_exit");
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
    let canary = pass_canary("slices/runtime_indexed_read_operand_exit");
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

#[test]
fn runtime_numeric_conversion_surface_exit_canary_runs() {
    let canary = pass_canary("core/numeric_conversion_surface");
    let build_dir =
        std::env::temp_dir().join(format!("omega-numeric-conversion-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("unsigned numeric conversion surface should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("unsigned numeric conversion surface should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected exact/wrapping/saturating/trapping/widening unsigned conversions to agree; \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_i64_to_u64_exact_guard_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_i64_to_u64_exact_guard_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-i64-u64-exact-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("guarded dynamic i64-to-u64 exact conversion should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("guarded i64-to-u64 canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("guarded dynamic i64-to-u64 exact conversion should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected guarded dynamic i64-to-u64 exact conversion to preserve 32; got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_numeric_signed_conversion_surface_exit_canary_runs() {
    let canary = pass_canary("core/numeric_signed_conversion_surface");
    let build_dir =
        std::env::temp_dir().join(format!("omega-numeric-signed-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("signed numeric conversion surface should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("signed numeric conversion surface should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected signed exact/wrapping/saturating/trapping/widening conversions to agree; \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn numeric_trapping_conversion_overflow_aborts() {
    let canary = pass_canary("core/numeric_trapping_conversion_overflow");
    let build_dir = std::env::temp_dir().join(format!("omega-numeric-trap-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("trapping numeric conversion should compile from its authored root");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("trapping numeric conversion should run");
    assert!(
        !output.status.success() && output.status.code() != Some(7),
        "expected out-of-range narrowing to trap before returning; got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("trapping numeric conversion should reach checked trees");
    let outcome = interpret(&checked, &[]);
    assert!(
        outcome
            .error
            .as_deref()
            .is_some_and(|reason| reason.contains("arithmetic overflow in Trapping domain")),
        "interpreter must report the same conversion trap, got {:?}",
        outcome.error
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_numeric_cross_signed_conversion_surface_exit_canary_runs() {
    let canary = pass_canary("core/numeric_cross_signed_conversion_surface");
    let build_dir =
        std::env::temp_dir().join(format!("omega-numeric-cross-signed-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("cross-signed numeric conversion surface should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("cross-signed numeric conversion surface should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected both signedness directions and all explicit policies to agree; \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("cross-signed surface should reach checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter must agree on cross-signed conversions, got {:?}",
        outcome.error
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn numeric_cross_signed_trapping_conversions_abort() {
    for (name, label) in [
        (
            "core/numeric_cross_signed_unsigned_overflow_traps",
            "unsigned upper half to signed",
        ),
        (
            "core/numeric_cross_signed_negative_traps",
            "negative signed value to unsigned",
        ),
    ] {
        let canary = pass_canary(name);
        let build_dir = std::env::temp_dir().join(format!(
            "omega-numeric-cross-trap-{}-{}",
            label.replace(' ', "-"),
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);

        compile_rooted_canary_for_native_host(&canary, build_dir.clone()).unwrap_or_else(
            |diagnostics| {
                panic!(
                    "{label} trapping conversion should compile:\n{}",
                    diagnostics
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            },
        );

        let output = Command::new(build_dir.join(executable_name()))
            .output()
            .unwrap_or_else(|error| panic!("{label} trapping conversion should run: {error}"));
        assert!(
            !output.status.success() && output.status.code() != Some(7),
            "{label} must trap before returning; got {:?}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );

        let checked = compile_to_checked(&canary.join("main.omg"), None)
            .unwrap_or_else(|_| panic!("{label} should reach checked trees"));
        let outcome = interpret(&checked, &[]);
        assert!(
            outcome
                .error
                .as_deref()
                .is_some_and(|reason| reason.contains("arithmetic overflow in Trapping domain")),
            "interpreter must report the same {label} trap, got {:?}",
            outcome.error
        );

        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn runtime_subslice_len_exit_canary_runs() {
    // A `&[u8]` bound to a literal fixed-array subslice (`self.source[0..2]`)
    // and used only for `.len` is inlined to `(self.source[0..2]).len`; the
    // length must FOLD to the window width `b - a` (2), not fall through to a
    // place read with no descriptor slot. Exits 70 when `s.len == 2`.
    let canary = pass_canary("slices/runtime_subslice_len_exit");
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
    let canary = pass_canary("slices/runtime_slice_index_read_dispatch_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-read-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime dispatch slice index read canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
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
    let canary = pass_canary("slices/runtime_slice_index_copy_exit");
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
    let canary = pass_canary("slices/runtime_slice_index_copy_dispatch_exit");
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
    let canary = pass_canary("slices/runtime_frame_array_slice_parameter_alias_exit");
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
    let canary = pass_canary("slices/runtime_slice_len_transition_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-len-transition-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime slice len transition canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
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
    let canary = pass_canary("slices/runtime_subslice_param_bounded_range_exit");
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
    let canary = pass_canary("slices/runtime_subslice_param_end_only_exit");
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
    let canary = pass_canary("slices/runtime_subslice_param_local_exit");
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
    let canary = pass_canary("slices/runtime_subslice_runtime_start_exit");
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
    let canary = pass_canary("slices/runtime_subslice_runtime_end_exit");
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
    let canary = pass_canary("slices/runtime_subslice_nested_of_param_exit");
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
    let canary = pass_canary("slices/runtime_subslice_runtime_start_over_local_exit");
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
    let canary = pass_canary("slices/runtime_subslice_param_inclusive_end_exit");
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
    let canary = pass_canary("slices/runtime_subslice_range_len_exit");
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
    let canary = pass_canary("slices/runtime_subslice_bounded_range_len_exit");
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
    let canary = pass_canary("slices/runtime_subslice_range_pointer_exit");
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

#[test]
fn runtime_local_aggregate_into_let_exit_canary_runs() {
    // A local ARRAY literal read by a subsequent `let` (`let arr = [..]; let e = arr[1]`)
    // silently yielded 0: the liveness scan never inspected LocalData (`let`) values, so
    // the read-only array was elided (no slot) and the indexed read resolved against a
    // missing slot. Fixed by keeping the slot for an array-literal local referenced in a
    // later let value (array-only -- borrow-carrying structs must stay folded).
    let canary = pass_canary("slices/runtime_local_aggregate_into_let_exit");
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
    let canary = pass_canary("slices/runtime_field_array_element_value_operand_exit");
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
    let canary = pass_canary("slices/runtime_subslice_dynamic_index_exit");
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
    let canary = pass_canary("slices/runtime_subslice_bounded_dynamic_index_exit");
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
    let canary = pass_canary("slices/runtime_subslice_end_dynamic_index_exit");
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
    let canary = pass_canary("slices/runtime_nested_subslice_dynamic_index_exit");
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
    let canary = pass_canary("slices/runtime_nested_subslice_fixed_index_exit");
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
    let canary = pass_canary("slices/runtime_slice_fixed_index_guard_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-fixed-index-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime slice fixed index guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
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
    let canary = pass_canary("slices/runtime_local_slice_len_comparison_value_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-local-slice-len-comparison-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime local slice len comparison canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
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
    let canary = pass_canary("slices/runtime_slice_index_transition_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-index-transition-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime slice index transition canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
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
    let canary = pass_canary("slices/runtime_slice_iteration_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-iteration-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime slice iteration canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
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

#[test]
fn runtime_string_concat_membership_exit_canary_runs() {
    let canary = pass_canary("text/runtime_string_concat_membership_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-string-concat-membership-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime string concat membership canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("string concat membership canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime string concat membership canary should run");

    assert_eq!(
        output.status.code(),
        Some(71),
        "expected runtime string concat membership canary to preserve concat result and exit 71, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_string_field_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_string_field_concat_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-string-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime string field concat canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("string field concat canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime string field concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(73),
        "expected runtime string field concat canary to preserve nested string writes and exit 73, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_owned_indexed_string_field_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_machine_owned_indexed_string_field_concat_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-indexed-string-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime machine-owned indexed string field concat canary should compile from its authored root",
    );

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime machine-owned indexed string field concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(81),
        "expected runtime machine-owned indexed string field concat canary to preserve direct machine-owned indexed string writes and exit 81, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_owned_indexed_bounded_carrier_literal_exit_canary_runs() {
    let canary = pass_canary("text/runtime_machine_owned_indexed_bounded_carrier_literal_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-indexed-bounded-carrier-literal-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime machine-owned indexed bounded-carrier literal canary should compile from its authored root",
    );

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime machine-owned indexed bounded-carrier literal canary should run");

    assert_eq!(
        output.status.code(),
        Some(85),
        "expected indexed owned-carrier literal assignment and append to preserve inline bytes and exit 85, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_owned_double_indexed_bounded_carrier_literal_exit_canary_runs() {
    let canary =
        pass_canary("text/runtime_machine_owned_double_indexed_bounded_carrier_literal_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-double-indexed-bounded-carrier-literal-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime machine-owned double-indexed bounded-carrier literal canary should compile from its authored root",
    );

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime machine-owned double-indexed bounded-carrier literal canary should run");

    assert_eq!(
        output.status.code(),
        Some(87),
        "expected double-indexed owned-carrier literal assignment and append to preserve inline bytes and exit 87, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_owned_double_indexed_string_field_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_machine_owned_double_indexed_string_field_concat_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-double-indexed-string-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime machine-owned double-indexed string field concat canary should compile from its authored root",
    );

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime machine-owned double-indexed string field concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(83),
        "expected runtime machine-owned double-indexed string field concat canary to preserve double-runtime-indexed string writes and exit 83, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_machine_owned_parameter_write_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_mutable_machine_owned_parameter_write_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-machine-owned-parameter-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime mutable machine-owned parameter write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("mutable machine-owned parameter canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable machine-owned parameter write canary should run");

    assert_eq!(
        output.status.code(),
        Some(141),
        "expected runtime mutable machine-owned parameter write canary to preserve writes through mutable machine-owned call parameters and exit 141, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_local_parameter_write_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_mutable_local_parameter_write_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-local-parameter-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime mutable local parameter write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("mutable local parameter canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable local parameter write canary should run");

    assert_eq!(
        output.status.code(),
        Some(171),
        "expected runtime mutable local parameter write canary to preserve writes through local mutable call parameters and exit 171, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_parameter_read_modify_write_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_mutable_parameter_read_modify_write_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-parameter-read-modify-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime mutable parameter read/modify/write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("mutable parameter RMW canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable parameter read/modify/write canary should run");

    assert_eq!(
        output.status.code(),
        Some(191),
        "expected runtime mutable parameter read/modify/write canary to preserve aliased binary writes and exit 191, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}
