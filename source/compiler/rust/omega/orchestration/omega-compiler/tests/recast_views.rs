//! Focused native canaries for programmable-layout recast views.
//!
//! These live outside the monolithic canary suite so each new view rung can
//! carry its own end-to-end oracle without making that shared file responsible
//! for another subsystem.

use omega_compiler::{
    CheckedCompilation, CompileOptions, CompileRequest, compile, compile_to_checked,
};
use psi_checked_interpreter::{InterpretOutcome, interpret_entry};
use std::path::{Path, PathBuf};
use std::process::Command;

fn interpret(checked: &CheckedCompilation, stdin: &[u8]) -> InterpretOutcome {
    interpret_entry(
        checked,
        checked
            .selected_program_entry_machine()
            .expect("recast fixture selects an exact ProgramEntry"),
        stdin,
    )
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(6)
        .expect(
            "omega-compiler lives under source/compiler/rust/omega/orchestration/omega-compiler",
        )
        .to_path_buf()
}

fn compile_pass_to_checked(main: &Path) -> CheckedCompilation {
    let profile = omega_target::TargetProfile::host();
    compile_to_checked(main, Some(profile.target_name()))
        .expect("recast pass fixture should reach checked trees")
}

fn compile_and_run(canary_rel: &str, tag: &str) -> std::process::Output {
    let profile = omega_target::TargetProfile::host();
    let canary = repo_root().join("tests/canaries/pass").join(canary_rel);
    let build_dir = std::env::temp_dir().join(format!("omega-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);

    compile(CompileRequest::new(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some(profile.target_name().to_owned()),
        write_output: true,
    }))
    .unwrap_or_else(|diagnostics| panic!("{canary_rel} should compile:\n{diagnostics:#?}"));

    let executable = if cfg!(windows) {
        "omega-program.exe"
    } else {
        "omega-program"
    };
    let output = Command::new(build_dir.join(executable))
        .output()
        .expect("canary should run");
    let _ = std::fs::remove_dir_all(&build_dir);
    output
}

fn assert_exit_70(canary_rel: &str, tag: &str) {
    let output = compile_and_run(canary_rel, tag);
    assert_eq!(
        output.status.code(),
        Some(70),
        "{canary_rel} expected exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn compile_for_cross_targets(canary_rel: &str, tag: &str) {
    let canary = repo_root().join("tests/canaries/pass").join(canary_rel);
    for target in ["windows_x64", "linux_arm64"] {
        let cross_dir =
            std::env::temp_dir().join(format!("omega-{tag}-{target}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&cross_dir);
        let source_dir = cross_dir.join("src");
        let build_dir = cross_dir.join("build");
        std::fs::create_dir_all(&source_dir).expect("create cross-target source directory");
        std::fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy recast canary");
        std::fs::copy(canary.join("build.omg"), source_dir.join("build.omg"))
            .expect("copy exact recast root matrix");

        compile(CompileRequest::new(CompileOptions {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(build_dir),
            target_name: Some(target.to_owned()),
            write_output: true,
        }))
        .unwrap_or_else(|diagnostics| {
            panic!(
                "{canary_rel} should compile for {target}:\n{}",
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
        let _ = std::fs::remove_dir_all(&cross_dir);
    }
}

fn fail_diagnostics(canary_rel: &str) -> String {
    let canary = repo_root()
        .join("tests/canaries/fail")
        .join(canary_rel)
        .join("main.omg");
    compile_to_checked(&canary, None)
        .expect_err("recast safety canary must reject")
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn scalar_and_interior_recast_execution_canaries_run() {
    for (canary, tag) in [
        (
            "recast/runtime_scalar_pun_shared_let_exit",
            "scalar-pun-shared",
        ),
        (
            "recast/runtime_interior_byte_recast_exit",
            "interior-byte-recast",
        ),
        (
            "recast/runtime_offset_byte_recast_exit",
            "offset-byte-recast",
        ),
    ] {
        assert_exit_70(canary, tag);
    }
}

#[test]
fn mutable_recast_execution_canaries_run() {
    for (canary, tag) in [
        (
            "recast/runtime_scalar_pun_mutable_write_exit",
            "mutable-scalar-pun",
        ),
        (
            "recast/runtime_offset_byte_recast_mutable_write_exit",
            "mutable-byte-region",
        ),
    ] {
        assert_exit_70(canary, tag);
    }
}

#[test]
fn mutable_recasts_cross_compile() {
    compile_for_cross_targets(
        "recast/runtime_scalar_pun_mutable_write_exit",
        "mutable-scalar-pun",
    );
    compile_for_cross_targets(
        "recast/runtime_offset_byte_recast_mutable_write_exit",
        "mutable-byte-region",
    );
}

#[test]
fn flow_proven_recast_execution_canaries_run() {
    for (canary, tag) in [
        (
            "recast/runtime_multi_edge_offset_meet_exit",
            "multi-edge-offset-meet",
        ),
        (
            "recast/runtime_guarded_offset_recast_exit",
            "guarded-offset-recast",
        ),
        (
            "recast/runtime_symbolic_stride_footprint_exit",
            "symbolic-stride-footprint",
        ),
    ] {
        assert_exit_70(canary, tag);
    }
}

#[test]
fn record_recast_execution_canaries_run() {
    for (canary, tag) in [
        ("recast/runtime_record_view_exit", "record-view"),
        (
            "recast/runtime_record_array_view_mutable_write_exit",
            "record-array-mutable-view",
        ),
        (
            "recast/constant_offset_record_view_after_write_exit",
            "constant-offset-record-view",
        ),
    ] {
        assert_exit_70(canary, tag);
    }

    let array_canary =
        repo_root().join("tests/canaries/pass/recast/runtime_record_array_view_mutable_write_exit");
    let checked = compile_pass_to_checked(&array_canary.join("main.omg"));
    assert_eq!(
        interpret(&checked, &[]).exit_code,
        70,
        "the interpreter must preserve nested array/record offsets"
    );
    compile_for_cross_targets(
        "recast/runtime_record_array_view_mutable_write_exit",
        "record-array-mutable-view",
    );
}

#[test]
fn fixed_array_recast_execution_and_fact_fence() {
    let canary = "recast/runtime_fixed_array_view_mutable_write_exit";
    assert_exit_70(canary, "fixed-array-mutable-view");

    let main = repo_root()
        .join("tests/canaries/pass")
        .join(canary)
        .join("main.omg");
    let checked = compile_pass_to_checked(&main);
    assert_eq!(
        interpret(&checked, &[]).exit_code,
        70,
        "the interpreter must preserve top-level fixed-array view identity"
    );

    compile_for_cross_targets(canary, "fixed-array-mutable-view");

    let diagnostics = fail_diagnostics("recast/fixed_array_view_fact_fenced");
    assert!(
        diagnostics.contains("must be recursively fact-free"),
        "raw bytes must not establish fixed-array element facts:\n{diagnostics}"
    );
}

#[test]
fn slice_recast_execution_tiling_and_fact_fences() {
    let canary = "recast/runtime_slice_view_mutable_write_exit";
    assert_exit_70(canary, "slice-mutable-view");

    let main = repo_root()
        .join("tests/canaries/pass")
        .join(canary)
        .join("main.omg");
    let checked = compile_pass_to_checked(&main);
    let interpreted = interpret(&checked, &[]);
    assert_eq!(
        interpreted.exit_code, 70,
        "the interpreter must derive slice length and preserve write-through: {interpreted:?}"
    );

    compile_for_cross_targets(canary, "slice-mutable-view");

    let non_tiling = fail_diagnostics("recast/slice_view_non_tiling_rejected");
    assert!(
        non_tiling.contains("does not exactly tile"),
        "non-divisible slice recast produced the wrong diagnostic:\n{non_tiling}"
    );
    let facted = fail_diagnostics("recast/slice_view_fact_fenced");
    assert!(
        facted.contains("raw storage cannot establish element facts"),
        "raw bytes must not establish slice element facts:\n{facted}"
    );
}

#[test]
fn interior_slice_recasts_preserve_dynamic_tail_geometry() {
    let canary = "recast/runtime_interior_slice_view_mutable_write_exit";
    assert_exit_70(canary, "interior-slice-mutable-view");

    let main = repo_root()
        .join("tests/canaries/pass")
        .join(canary)
        .join("main.omg");
    let checked = compile_pass_to_checked(&main);
    let interpreted = interpret(&checked, &[]);
    assert_eq!(
        interpreted.exit_code, 70,
        "the interpreter must preserve interior slice length and write-through: {interpreted:?}"
    );

    compile_for_cross_targets(canary, "interior-slice-mutable-view");

    let non_tiling = fail_diagnostics("recast/interior_slice_runtime_offset_non_tiling");
    assert!(
        non_tiling.contains("cannot prove exact tiling for interior slice"),
        "runtime-offset tiling produced the wrong diagnostic:\n{non_tiling}"
    );
    let facted = fail_diagnostics("recast/interior_slice_fact_fenced");
    assert!(
        facted.contains("raw storage cannot establish element facts"),
        "interior raw bytes must not establish slice element facts:\n{facted}"
    );
}

#[test]
fn aggregate_slice_recasts_compose_leaf_representation_sets() {
    let canary = "recast/runtime_aggregate_slice_representation_recast_exit";
    assert_exit_70(canary, "aggregate-slice-representation-recast");

    let main = repo_root()
        .join("tests/canaries/pass")
        .join(canary)
        .join("main.omg");
    let checked = compile_pass_to_checked(&main);
    let interpreted = interpret(&checked, &[]);
    assert_eq!(
        interpreted.exit_code, 70,
        "the interpreter must preserve aggregate slice facts and write-through: {interpreted:?}"
    );

    compile_for_cross_targets(canary, "aggregate-slice-representation-recast");

    let diagnostics = fail_diagnostics("recast/aggregate_slice_mut_leaf_sets_differ");
    assert!(
        diagnostics.contains("fact implication in BOTH directions"),
        "aggregate slice leaf-set mismatch produced the wrong diagnostic:\n{diagnostics}"
    );
}

#[test]
fn mutable_recast_fact_fences_reject() {
    for canary in [
        "recast/recast_mut_fact_fenced",
        "recast/recast_mut_interior_fact_fenced",
        "recast/recast_mut_record_fact_fenced",
        "recast/recast_mut_record_array_fact_fenced",
    ] {
        let diagnostics = fail_diagnostics(canary);
        assert!(
            diagnostics.contains("fact implication in BOTH directions"),
            "{canary} produced the wrong mutable-recast diagnostic:\n{diagnostics}"
        );
    }
}

#[test]
fn mutable_recast_accepts_bidirectionally_equivalent_domain_facts() {
    let canary = "recast/runtime_mutable_equivalent_domain_recast_exit";
    assert_exit_70(canary, "mutable-equivalent-domain-recast");
}

#[test]
fn mutable_recast_accepts_equal_integer_representation_sets() {
    let canary = "recast/runtime_mutable_equivalent_range_recast_exit";
    assert_exit_70(canary, "mutable-equivalent-range-recast");
    compile_for_cross_targets(canary, "mutable-equivalent-range-recast");
}

#[test]
fn scalar_bool_recasts_follow_representation_set_implication() {
    let canary = "recast/runtime_bool_representation_recast_exit";
    assert_exit_70(canary, "bool-representation-recast");
    compile_for_cross_targets(canary, "bool-representation-recast");

    for fail in [
        "recast/recast_shared_bool_fact_fenced",
        "recast/recast_shared_interior_fact_fenced",
    ] {
        let diagnostics = fail_diagnostics(fail);
        assert!(
            diagnostics.contains("may weaken established facts but cannot strengthen them"),
            "{fail} produced the wrong shared representation-set diagnostic:\n{diagnostics}"
        );
    }
    let diagnostics = fail_diagnostics("recast/recast_mut_bool_bit_sets_differ");
    assert!(
        diagnostics.contains("fact implication in BOTH directions"),
        "mutable bool/full-byte alias produced the wrong diagnostic:\n{diagnostics}"
    );
}

#[test]
fn shared_domain_recasts_require_one_way_implication() {
    let canary = "recast/runtime_shared_domain_weakening_recast_exit";
    assert_exit_70(canary, "shared-domain-weakening-recast");
    compile_for_cross_targets(canary, "shared-domain-weakening-recast");

    let diagnostics = fail_diagnostics("recast/recast_shared_domain_strengthening_rejected");
    assert!(
        diagnostics.contains("may weaken established facts but cannot strengthen them"),
        "shared domain strengthening produced the wrong diagnostic:\n{diagnostics}"
    );
}

#[test]
fn float_range_recasts_require_same_carrier_interval_implication() {
    let canary = "recast/runtime_float_range_representation_recast_exit";
    assert_exit_70(canary, "float-range-representation-recast");
    compile_for_cross_targets(canary, "float-range-representation-recast");

    let diagnostics = fail_diagnostics("recast/recast_shared_float_range_strengthening_rejected");
    assert!(
        diagnostics.contains("may weaken established facts but cannot strengthen them"),
        "shared float-range strengthening produced the wrong diagnostic:\n{diagnostics}"
    );

    let diagnostics = fail_diagnostics("recast/recast_mut_float_range_fenced");
    assert!(
        diagnostics
            .contains("source and target constraints are not proven representation-equivalent"),
        "cross-carrier mutable float range produced the wrong diagnostic:\n{diagnostics}"
    );
}

#[test]
fn record_recasts_compose_same_carrier_float_leaf_intervals() {
    let canary = "recast/runtime_shared_record_float_range_weakening_exit";
    assert_exit_70(canary, "shared-record-float-range-weakening");
    compile_for_cross_targets(canary, "shared-record-float-range-weakening");

    for fail in [
        "recast/recast_shared_record_float_leaf_strengthening_rejected",
        "recast/recast_mut_record_float_leaf_sets_differ",
    ] {
        let diagnostics = fail_diagnostics(fail);
        assert!(
            diagnostics.contains(if fail.contains("shared") {
                "source leaf facts implying every target leaf fact"
            } else {
                "leaf fact implication in BOTH directions"
            }),
            "{fail} produced the wrong record-float diagnostic:\n{diagnostics}"
        );
    }
}

#[test]
fn mutable_recast_accepts_equivalent_typed_record_representations() {
    let canary = "recast/runtime_mutable_equivalent_record_recast_exit";
    assert_exit_70(canary, "mutable-equivalent-record-recast");
    compile_for_cross_targets(canary, "mutable-equivalent-record-recast");
}

#[test]
fn mutable_recast_rejects_equal_looking_cross_carrier_domains() {
    let diagnostics = fail_diagnostics("recast/recast_mut_cross_carrier_domain_not_equivalent");
    assert!(
        diagnostics
            .contains("source and target constraints are not proven representation-equivalent"),
        "wrong cross-carrier recast diagnostic:\n{diagnostics}"
    );
}

#[test]
fn mutable_recast_rejects_different_range_bit_sets() {
    let diagnostics = fail_diagnostics("recast/recast_mut_range_bit_sets_differ");
    assert!(
        diagnostics
            .contains("source and target constraints are not proven representation-equivalent"),
        "wrong range representation-set diagnostic:\n{diagnostics}"
    );
}

#[test]
fn mutable_recast_does_not_treat_float_ranges_as_bit_pattern_sets() {
    let diagnostics = fail_diagnostics("recast/recast_mut_float_range_fenced");
    assert!(
        diagnostics
            .contains("source and target constraints are not proven representation-equivalent"),
        "wrong float-range representation diagnostic:\n{diagnostics}"
    );
}

#[test]
fn mutable_recast_rejects_different_record_leaf_sets() {
    let diagnostics = fail_diagnostics("recast/recast_mut_record_leaf_sets_differ");
    assert!(
        diagnostics.contains("identical layout geometry")
            && diagnostics.contains("fact implication in BOTH directions"),
        "wrong record representation-set diagnostic:\n{diagnostics}"
    );
}
