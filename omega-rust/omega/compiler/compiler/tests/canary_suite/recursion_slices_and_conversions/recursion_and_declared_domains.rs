use super::fixture_roster;
use crate::{
    Command, compile_reviewed_repository_fixture, compile_rooted_canary_for_native_host,
    executable_name, fs, native_hosted_target, pass_canary,
};
use compiler::CheckedCompileRequest;

#[test]
fn runtime_nat_structural_recursion_exit_canary_runs() {
    // N2(d) gateway: a free machine over proof-only Nat is a PROOF MACHINE
    // -- structural recursion legal, measured, every self-call descending
    // by a case-payload subterm; the program lowers without it.
    let canary = pass_canary(fixture_roster::RUNTIME_NAT_STRUCTURAL_RECURSION_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_CORE_NAT_DECLARED_EXIT);
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
    // citing it proves through the accepted fact. Runs untouched. The
    // ungranted axiom is an own-package dev-active accepted fact carrying the
    // standing warning in the trust report the compiler reconstructs from
    // the checked program during admission.
    let canary = pass_canary(fixture_roster::ACCEPTED_AXIOM_CITED_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-accepted-axiom-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
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
    let _ = fs::remove_dir_all(&build_dir);

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        Some(native_hosted_target()),
    ))
    .expect("accepted-axiom canary should reach checked semantics");
    let axiom = checked
        .terminal_production_trees()
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "mul_comm_axiom")
        .expect("the accepted axiom should reach checked semantics");
    assert!(!axiom.body_is_present, "an accepted axiom is bodyless");
    assert!(
        checked.root_grants().is_empty(),
        "the canary's build grants nothing, so the axiom stays dev-active"
    );
    let trust_report = trust_model::reconstruct_trust_report(
        checked.terminal_production_trees(),
        checked.root_grants(),
        checked.provider_plans(),
        checked.selected_provider_plans(),
        checked.accepted_template_classifications(),
    )
    .expect("the trust report reconstructs from the checked program");
    let commitments = trust_report
        .rows
        .iter()
        .map(|row| row.commitment.as_str())
        .collect::<Vec<_>>();
    let row = trust_report
        .rows
        .iter()
        .find(|row| row.commitment == "accepted fact: mul_comm_axiom")
        .unwrap_or_else(|| panic!("the axiom must surface as a trust row:\n{commitments:#?}"));
    assert_eq!(
        row.provenance, "own-package (dev-active)",
        "an ungranted axiom is own-package dev-active"
    );
    assert!(
        row.standing_warning,
        "an ungranted axiom is dev-active with the standing warning"
    );
}

#[test]
fn runtime_core_rat_declared_exit_canary_runs() {
    // N4 Rat rung: the canonical-representative Rat carrier loads through
    // the bundled core; proof-only data over Nat never touches runtime.
    let canary = pass_canary(fixture_roster::RUNTIME_CORE_RAT_DECLARED_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_FREE_CONST_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_TERMINAL_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-native-call-{}", std::process::id()));
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
    let canary = pass_canary(fixture_roster::RUNTIME_RESULT_DOMAIN_MACHINE_OVERLOAD_EXIT);
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
fn runtime_result_domain_attached_overload_exit_canary_runs() {
    // Member-receiver selection on an attached result-overload family: each
    // `self.helper.pick()` must rebind to the overload matching the typed LET
    // destination's dispatch set (70 qualified, 69 empty) or exit 77.
    let canary = pass_canary(fixture_roster::RUNTIME_RESULT_DOMAIN_ATTACHED_OVERLOAD_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-attached-result-overload-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("attached result-domain overload canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("attached result overload canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("attached result-domain overload canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected member calls to select distinct attached overloads, got {:?}\n{}",
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
    let canary = pass_canary(fixture_roster::RUNTIME_STD_MATH_SIN_COS_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_COMPUTED_INDEX_MATCH_SUBJECT_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_CONST_MEASURED_RECURSION_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_TERMINAL_TAIL_RECURSION_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-native-tail-{}", std::process::id()));
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
    let canary = pass_canary(fixture_roster::RUNTIME_MEASURED_TAIL_RECURSION_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_U64_GUARDED_CAP_STORE_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_PROOF_ONLY_DATA_DECLARED_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_F32_FIELD_GUARD_EXIT);
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
