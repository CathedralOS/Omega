//! Focused native canaries for programmable-layout recast views.
//!
//! These live outside the monolithic canary suite so each new view rung can
//! carry its own end-to-end oracle without making that shared file responsible
//! for another subsystem.

use omega_compiler::{CompileOptions, compile, compile_to_checked};
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("omega-compiler lives under compiler/orchestration/omega-compiler")
        .to_path_buf()
}

fn compile_and_run(canary_rel: &str, tag: &str) -> std::process::Output {
    let canary = repo_root().join("canaries/pass").join(canary_rel);
    let build_dir = std::env::temp_dir().join(format!("omega-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|diagnostics| panic!("{canary_rel} should compile:\n{diagnostics:#?}"));

    let output = Command::new(build_dir.join("omega-program"))
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
    let canary = repo_root().join("canaries/pass").join(canary_rel);
    for target in ["windows_x64", "linux_arm64"] {
        let cross_dir =
            std::env::temp_dir().join(format!("omega-{tag}-{target}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&cross_dir);
        let source_dir = cross_dir.join("src");
        let build_dir = cross_dir.join("build");
        std::fs::create_dir_all(&source_dir).expect("create cross-target source directory");
        std::fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy recast canary");
        std::fs::write(
            source_dir.join("build.omg"),
            format!("target {target} {{\n}}\n"),
        )
        .expect("write cross-target manifest");

        compile(CompileOptions {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(build_dir),
            target_name: Some(target.to_owned()),
            write_output: true,
        })
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
        .join("canaries/fail")
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
        repo_root().join("canaries/pass/recast/runtime_record_array_view_mutable_write_exit");
    let checked = compile_to_checked(&array_canary.join("main.omg"), None)
        .expect("mutable record-array view should compile to checked trees");
    assert_eq!(
        omega_interpreter::interpret(&checked, &[]).exit_code,
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
        .join("canaries/pass")
        .join(canary)
        .join("main.omg");
    let checked =
        compile_to_checked(&main, None).expect("top-level fixed-array view should compile");
    assert_eq!(
        omega_interpreter::interpret(&checked, &[]).exit_code,
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
