//! End-to-end oracles for plan-laid outer fixed arrays whose validated
//! destinations retain a constant physical stride larger than element width.

use omega_compiler::{
    BuildTimeValue, CompileOptions, compile, compile_to_checked, compute_layout_plan,
    evaluate_and_materialize_typed_owned_layout_into, materialize_typed_owned_layout_into,
};
use psi_checked_interpreter::interpret_entry;
use psi_layout_plans::ByteOrder;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const PRIMITIVE_CANARY: &str = "layouts/runtime_plan_laid_tiled_outer_array_view_exit";
const RECORD_CANARY: &str = "layouts/runtime_plan_laid_tiled_record_array_view_exit";
const NESTED_ARRAY_CANARY: &str = "layouts/runtime_plan_laid_tiled_nested_array_view_exit";
const RECORD_NESTED_ARRAY_CANARY: &str =
    "layouts/runtime_plan_laid_tiled_record_nested_array_view_exit";
const MULTIPLE_AGGREGATE_FIELDS_CANARY: &str =
    "layouts/runtime_plan_laid_multiple_aggregate_fields_view_exit";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(6)
        .expect("omega-compiler lives under bootstrap/onramps/omega-rust/omega/orchestration/omega-compiler")
        .to_path_buf()
}

struct TemporaryBuildDirectory(PathBuf);

impl TemporaryBuildDirectory {
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryBuildDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn unique_build_dir(tag: &str) -> TemporaryBuildDirectory {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should follow the Unix epoch")
        .as_nanos();
    TemporaryBuildDirectory(
        std::env::temp_dir().join(format!("omega-{tag}-{}-{nonce}", std::process::id())),
    )
}

fn assert_runtime_canary(canary_name: &str, tag: &str) {
    let canary = repo_root().join("canaries/pass").join(canary_name);
    let host = omega_target::TargetProfile::host();
    let checked = compile_to_checked(&canary.join("main.omg"), Some(host.target_name()))
        .expect("gapped outer-array canary should reach checked trees");
    let interpreted = interpret_entry(
        &checked,
        checked
            .selected_program_entry_machine()
            .expect("gapped outer-array canary selects an exact ProgramEntry"),
        &[],
    );
    assert_eq!(
        interpreted.exit_code, 70,
        "interpreter must preserve the validated element stride: {interpreted:?}"
    );

    let host_build = unique_build_dir(&format!("{tag}-host"));
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(host_build.path().to_path_buf()),
        target_name: Some(host.target_name().to_owned()),
        write_output: true,
    })
    .unwrap_or_else(|diagnostics| panic!("host compile should succeed:\n{diagnostics:#?}"));
    let executable = if cfg!(windows) {
        "omega-program.exe"
    } else {
        "omega-program"
    };
    let output = Command::new(host_build.path().join(executable))
        .output()
        .expect("gapped outer-array executable should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "native gapped outer-array canary failed; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let host_build_path = host_build.path().to_path_buf();
    drop(host_build);
    assert!(
        !host_build_path.exists(),
        "host build directory should be removed after execution"
    );

    for target in ["windows_x64", "linux_arm64"] {
        let cross_build = unique_build_dir(&format!("{tag}-{target}"));
        compile(CompileOptions {
            root_path: canary.join("main.omg"),
            build_dir: Some(cross_build.path().to_path_buf()),
            target_name: Some(target.to_owned()),
            write_output: true,
        })
        .unwrap_or_else(|diagnostics| panic!("{target} compile should succeed:\n{diagnostics:#?}"));
        let cross_build_path = cross_build.path().to_path_buf();
        drop(cross_build);
        assert!(
            !cross_build_path.exists(),
            "{target} build directory should be removed after compilation"
        );
    }
}

#[test]
fn gapped_primitive_outer_array_interprets_runs_natively_and_cross_compiles() {
    assert_runtime_canary(PRIMITIVE_CANARY, "plan-laid-gapped-primitive");
}

#[test]
fn gapped_record_outer_array_interprets_runs_natively_and_cross_compiles() {
    assert_runtime_canary(RECORD_CANARY, "plan-laid-gapped-record");
}

#[test]
fn gapped_nested_array_outer_array_interprets_runs_natively_and_cross_compiles() {
    assert_runtime_canary(NESTED_ARRAY_CANARY, "plan-laid-gapped-nested-array");
}

#[test]
fn gapped_record_nested_array_outer_array_interprets_runs_natively_and_cross_compiles() {
    assert_runtime_canary(
        RECORD_NESTED_ARRAY_CANARY,
        "plan-laid-gapped-record-nested-array",
    );
}

#[test]
fn multiple_whole_aggregate_fields_interpret_run_natively_and_cross_compile() {
    assert_runtime_canary(
        MULTIPLE_AGGREGATE_FIELDS_CANARY,
        "plan-laid-multiple-aggregate-fields",
    );
}

#[test]
fn gapped_record_outer_array_materializes_from_checked_owned_value() {
    let canary = repo_root().join("canaries/pass").join(RECORD_CANARY);
    let host = omega_target::TargetProfile::host();
    let checked = compile_to_checked(&canary.join("main.omg"), Some(host.target_name()))
        .expect("gapped record-array canary should reach checked trees");
    let layout = compute_layout_plan(&checked.typed, "TiledRecordArray::plan", "Samples")
        .expect("gapped record-array plan should validate");

    let mut little = [0xa5; 24];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_samples",
        "Samples",
        &layout,
        ByteOrder::LittleEndian,
        &mut little,
    )
    .expect("checked owned record array should materialize little-endian");
    assert_eq!(
        little,
        [
            0, 0, 0, 0, 1, 2, 0, 0, 3, 4, 5, 6, 0, 0, 0, 0, 7, 8, 0, 0, 9, 10, 11, 12,
        ]
    );

    let mut big = [0xa5; 24];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_samples",
        "Samples",
        &layout,
        ByteOrder::BigEndian,
        &mut big,
    )
    .expect("checked owned record array should materialize big-endian");
    assert_eq!(
        big,
        [
            0, 0, 0, 0, 2, 1, 0, 0, 6, 5, 4, 3, 0, 0, 0, 0, 8, 7, 0, 0, 12, 11, 10, 9,
        ]
    );
}

#[test]
fn gapped_nested_array_outer_array_materializes_from_checked_owned_value() {
    let canary = repo_root().join("canaries/pass").join(NESTED_ARRAY_CANARY);
    let host = omega_target::TargetProfile::host();
    let checked = compile_to_checked(&canary.join("main.omg"), Some(host.target_name()))
        .expect("gapped nested-array canary should reach checked trees");
    let layout = compute_layout_plan(&checked.typed, "TiledNestedArray::plan", "Samples")
        .expect("gapped nested-array plan should validate");

    let mut little = [0xa5; 20];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_samples",
        "Samples",
        &layout,
        ByteOrder::LittleEndian,
        &mut little,
    )
    .expect("checked owned nested array should materialize little-endian");
    assert_eq!(
        little,
        [0, 0, 0, 0, 1, 2, 3, 4, 0, 0, 0, 0, 5, 6, 7, 8, 0, 0, 0, 0,]
    );

    let mut big = [0xa5; 20];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_samples",
        "Samples",
        &layout,
        ByteOrder::BigEndian,
        &mut big,
    )
    .expect("checked owned nested array should materialize big-endian");
    assert_eq!(
        big,
        [0, 0, 0, 0, 2, 1, 4, 3, 0, 0, 0, 0, 6, 5, 8, 7, 0, 0, 0, 0,]
    );
}

#[test]
fn gapped_record_nested_array_materialization_is_exact_and_atomic() {
    let canary = repo_root()
        .join("canaries/pass")
        .join(RECORD_NESTED_ARRAY_CANARY);
    let host = omega_target::TargetProfile::host();
    let checked = compile_to_checked(&canary.join("main.omg"), Some(host.target_name()))
        .expect("gapped record-nested-array canary should reach checked trees");
    let layout = compute_layout_plan(&checked.typed, "TiledRecordNestedArray::plan", "Samples")
        .expect("gapped record-nested-array plan should validate");

    let mut little = [0xa5; 28];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_samples",
        "Samples",
        &layout,
        ByteOrder::LittleEndian,
        &mut little,
    )
    .expect("checked owned record-nested array should materialize little-endian");
    assert_eq!(
        little,
        [
            0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0, 0, 9, 10, 11, 12, 13, 14, 15, 16, 0, 0, 0,
            0,
        ]
    );

    let mut big = [0xa5; 28];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_samples",
        "Samples",
        &layout,
        ByteOrder::BigEndian,
        &mut big,
    )
    .expect("checked owned record-nested array should materialize big-endian");
    assert_eq!(
        big,
        [
            0, 0, 0, 0, 4, 3, 2, 1, 6, 5, 8, 7, 0, 0, 0, 0, 12, 11, 10, 9, 14, 13, 16, 15, 0, 0, 0,
            0,
        ]
    );

    let cell = |type_name: &str, tag: i64, values: Vec<BuildTimeValue>| BuildTimeValue::Struct {
        type_name: type_name.to_owned(),
        fields: vec![
            ("tag".to_owned(), BuildTimeValue::Int(tag.into())),
            ("values".to_owned(), BuildTimeValue::Array(values)),
        ],
    };
    let short_inner = BuildTimeValue::Struct {
        type_name: "Samples".to_owned(),
        fields: vec![(
            "cells".to_owned(),
            BuildTimeValue::Array(vec![
                cell(
                    "Cell",
                    0x04030201,
                    vec![
                        BuildTimeValue::Int(0x0605.into()),
                        BuildTimeValue::Int(0x0807.into()),
                    ],
                ),
                cell("Cell", 0x0c0b0a09, vec![BuildTimeValue::Int(0x0e0d.into())]),
            ]),
        )],
    };
    let mut unchanged = [0x5a; 28];
    let error = materialize_typed_owned_layout_into(
        &checked.typed,
        "Samples",
        &layout,
        &short_inner,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("a short interior array must reject before destination mutation");
    assert!(error.0.contains("has 1 elements, expected 2"));
    assert_eq!(unchanged, [0x5a; 28]);

    let wrong_record = BuildTimeValue::Struct {
        type_name: "Samples".to_owned(),
        fields: vec![(
            "cells".to_owned(),
            BuildTimeValue::Array(vec![
                cell(
                    "Cell",
                    0x04030201,
                    vec![
                        BuildTimeValue::Int(0x0605.into()),
                        BuildTimeValue::Int(0x0807.into()),
                    ],
                ),
                cell(
                    "Samples",
                    0x0c0b0a09,
                    vec![
                        BuildTimeValue::Int(0x0e0d.into()),
                        BuildTimeValue::Int(0x100f.into()),
                    ],
                ),
            ]),
        )],
    };
    let mut unchanged = [0x5a; 28];
    let error = materialize_typed_owned_layout_into(
        &checked.typed,
        "Samples",
        &layout,
        &wrong_record,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("a wrong nested record identity must reject before destination mutation");
    assert!(error.0.contains("does not match `Cell`"));
    assert_eq!(unchanged, [0x5a; 28]);
}

#[test]
fn multiple_whole_aggregate_fields_materialize_by_key_and_reject_atomically() {
    let canary = repo_root()
        .join("canaries/pass")
        .join(MULTIPLE_AGGREGATE_FIELDS_CANARY);
    let host = omega_target::TargetProfile::host();
    let checked = compile_to_checked(&canary.join("main.omg"), Some(host.target_name()))
        .expect("multiple-aggregate-fields canary should reach checked trees");
    let layout = compute_layout_plan(&checked.typed, "MultipleAggregateFields::plan", "Samples")
        .expect("multiple whole aggregate fields should validate");

    let mut little = [0xa5; 28];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_samples",
        "Samples",
        &layout,
        ByteOrder::LittleEndian,
        &mut little,
    )
    .expect("multiple whole aggregate fields should materialize little-endian");
    assert_eq!(
        little,
        [
            0, 0, 0, 0, 5, 6, 0, 0, 7, 8, 9, 10, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 0, 0, 0, 0,
        ]
    );

    let mut big = [0xa5; 28];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_samples",
        "Samples",
        &layout,
        ByteOrder::BigEndian,
        &mut big,
    )
    .expect("multiple whole aggregate fields should materialize big-endian");
    assert_eq!(
        big,
        [
            0, 0, 0, 0, 6, 5, 0, 0, 10, 9, 8, 7, 0, 0, 0, 0, 0, 0, 0, 0, 2, 1, 4, 3, 0, 0, 0, 0,
        ]
    );

    let valid_values = BuildTimeValue::Array(vec![
        BuildTimeValue::Int(0x0201.into()),
        BuildTimeValue::Int(0x0403.into()),
    ]);
    let wrong_nested_record = BuildTimeValue::Struct {
        type_name: "Samples".to_owned(),
        fields: vec![
            ("values".to_owned(), valid_values.clone()),
            (
                "pair".to_owned(),
                BuildTimeValue::Struct {
                    type_name: "Samples".to_owned(),
                    fields: vec![
                        ("low".to_owned(), BuildTimeValue::Int(0x0605.into())),
                        ("high".to_owned(), BuildTimeValue::Int(0x0a090807.into())),
                    ],
                },
            ),
        ],
    };
    let mut unchanged = [0x5a; 28];
    let error = materialize_typed_owned_layout_into(
        &checked.typed,
        "Samples",
        &layout,
        &wrong_nested_record,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("a wrong second aggregate identity must reject before any field commits");
    assert!(error.0.contains("does not match `Pair`"));
    assert_eq!(unchanged, [0x5a; 28]);

    let missing_nested_record = BuildTimeValue::Struct {
        type_name: "Samples".to_owned(),
        fields: vec![("values".to_owned(), valid_values)],
    };
    let mut unchanged = [0x5a; 28];
    let error = materialize_typed_owned_layout_into(
        &checked.typed,
        "Samples",
        &layout,
        &missing_nested_record,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("a missing aggregate field must reject before any field commits");
    assert!(
        error.0.contains("has 1 fields, expected 2"),
        "unexpected missing-field diagnostic: {}",
        error.0
    );
    assert_eq!(unchanged, [0x5a; 28]);
}
