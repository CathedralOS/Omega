//! Focused native canaries for programmable-layout recast views.
//!
//! These live outside the monolithic canary suite so each new view rung can
//! carry its own end-to-end oracle without making that shared file responsible
//! for another subsystem.

use build_declarations::{BuildDeclaration, extract_build_declaration};
use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
#[path = "support/console_acceptance.rs"]
mod console_acceptance;
#[path = "fixture_rosters/recast_views.rs"]
mod fixture_roster;
#[path = "support/linux_entry_acceptance.rs"]
mod linux_entry_acceptance;
#[path = "support/macos_entry_acceptance.rs"]
mod macos_entry_acceptance;
#[path = "support/windows_entry_acceptance.rs"]
mod windows_entry_acceptance;

use checked_interpreter::{InterpretOutcome, interpret_entry};
use compiler::CheckedCompileRequest;
use compiler::{CheckedCompilation, CompileOptions, compile_to_checked};
use diagnostics::Diagnostic;
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn interpret(checked: &CheckedCompilation, stdin: &[u8]) -> InterpretOutcome {
    interpret_entry(
        checked,
        BuildMachineEntry::Name(
            checked
                .selected_program_entry_machine()
                .expect("recast fixture selects an exact ProgramEntry"),
        ),
        stdin,
        InterpretOptions::default(),
    )
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler lives under omega-rust/omega/compiler/compiler")
        .to_path_buf()
}

fn fixture_package_identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32])
        .expect("recast fixture package identity is nonzero")
}

fn fixture_declares_ordinary_std(project_root: &Path) -> bool {
    fs::read_to_string(project_root.join("build.omg")).is_ok_and(|build| {
        build.contains("builder.depend(Source::Path") && build.contains("source/library/std")
    })
}

/// Package inputs for a recast fixture that declares the ordinary std
/// dependency in its `build.omg`; `None` for self-contained fixtures.
///
/// The package-aware source route never reads the `Source::Path` row: it
/// consumes the reconciled graph supplied here, which binds the root package
/// to the fixture's project directory and std to the repository path
/// directly. This matters for `compile_for_cross_targets`, which copies
/// `main.omg`/`build.omg` into a temporary directory where the checked-in
/// relative dependency location would dangle.
fn fixture_package_inputs(root_path: &Path) -> Option<PackageCompilationInputs> {
    let project_root = root_path
        .parent()
        .expect("recast fixture source has a project root");
    if !fixture_declares_ordinary_std(project_root) {
        return None;
    }

    let declaration = extract_build_declaration(project_root)
        .unwrap_or_else(|error| panic!("recast fixture {}: {error}", project_root.display()));
    let root_role = declaration.kind();
    let root_name = match declaration {
        BuildDeclaration::Application(application) => application.name,
        BuildDeclaration::Package(package) => package.name,
        BuildDeclaration::Workspace(_) => {
            panic!(
                "recast fixture {} cannot be a workspace root",
                project_root.display()
            )
        }
    };
    let root_identity = fixture_package_identity(1);
    let standard_library_identity = fixture_package_identity(2);
    let packages = vec![
        PackageSourceBinding::new(
            root_identity,
            root_name.into_string(),
            project_root.to_path_buf(),
        ),
        PackageSourceBinding::new(
            standard_library_identity,
            "omega-language-std",
            repo_root().join("source/library/std"),
        ),
    ];
    let dependencies = vec![PackageDependencyBinding::new(
        root_identity,
        "omega_language_std",
        standard_library_identity,
    )];

    Some(
        PackageCompilationInputs::new(root_identity, root_role, packages, dependencies)
            .unwrap_or_else(|errors| {
                panic!("recast fixture {}: {errors:#?}", project_root.display())
            }),
    )
}

fn fixture_accepts_console_exit(root_path: &Path) -> bool {
    fs::read_to_string(root_path).is_ok_and(|source| {
        source.contains("omega_language_std::console") && source.contains(".exit_process(")
    })
}

/// The package graph plus this harness's test acceptance of the exact
/// standard-library entry schema and Console provider the fixture selects.
/// Source spelling selects only this repository's test policy; every admitted
/// row is derived from and replayed against the preliminary checked graph.
/// This is test-owned acceptance, not evidence that an audit occurred.
fn reviewed_fixture_package_inputs(
    root_path: &Path,
    target_name: Option<&str>,
) -> Result<Option<PackageCompilationInputs>, Vec<Diagnostic>> {
    let Some(mut package_inputs) = fixture_package_inputs(root_path) else {
        return Ok(None);
    };
    let standard_library_root = repo_root().join("source/library/std");
    let standard_library = fixture_package_identity(2);
    let mut bindings = Vec::new();
    match target_name {
        Some("macos_arm64") => {
            bindings.push(macos_entry_acceptance::candidate_macos_entry_binding(
                &standard_library_root,
                standard_library,
            )?)
        }
        Some("linux_x86_64") => bindings.push(
            linux_entry_acceptance::candidate_linux_x86_64_entry_binding(
                &standard_library_root,
                standard_library,
            )?,
        ),
        Some("linux_arm64") => {
            bindings.push(linux_entry_acceptance::candidate_linux_arm64_entry_binding(
                &standard_library_root,
                standard_library,
            )?)
        }
        Some("windows_x86_64") => bindings.push(
            windows_entry_acceptance::candidate_windows_x86_64_entry_binding(
                &standard_library_root,
                standard_library,
            )?,
        ),
        _ => {}
    }
    if !bindings.is_empty() {
        package_inputs = package_inputs
            .with_accepted_semantic_bindings(bindings.clone())
            .map_err(|errors| {
                vec![Diagnostic::error(format!(
                    "recast fixture entry acceptance: {errors:?}"
                ))]
            })?;
    }
    if !fixture_accepts_console_exit(root_path) {
        return Ok(Some(package_inputs));
    }
    let preliminary = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs.clone()),
        ..CheckedCompileRequest::new(root_path, target_name)
    })?;
    bindings.push(console_acceptance::candidate_console_exit_binding(
        &preliminary,
        standard_library,
        false,
        false,
    )?);
    package_inputs
        .with_accepted_semantic_bindings(bindings)
        .map(Some)
        .map_err(|errors| {
            vec![Diagnostic::error(format!(
                "cannot admit recast fixture semantic binding: {errors:?}"
            ))]
        })
}

fn compile_pass_to_checked(main: &Path) -> CheckedCompilation {
    let profile = target::TargetProfile::host();
    let mut request = CheckedCompileRequest::new(main, Some(profile.target_name()));
    request.package_inputs = reviewed_fixture_package_inputs(main, Some(profile.target_name()))
        .unwrap_or_else(|diagnostics| {
            panic!("recast pass fixture package inputs:\n{diagnostics:#?}")
        });
    compile_to_checked(request).expect("recast pass fixture should reach checked trees")
}

fn compile_and_run(canary_rel: &str, tag: &str) -> std::process::Output {
    let profile = target::TargetProfile::host();
    let canary = repo_root().join("tests/omega/pass").join(canary_rel);
    let root_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);

    let package_inputs = reviewed_fixture_package_inputs(&root_path, Some(profile.target_name()))
        .unwrap_or_else(|diagnostics| panic!("{canary_rel} package inputs:\n{diagnostics:#?}"));
    let mut request = compiler::CompileRequest::new(CompileOptions {
        root_path,
        build_dir: Some(build_dir.clone()),
        target_name: Some(profile.target_name().to_owned()),
    })
    .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact);
    if let Some(package_inputs) = package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    let report = compiler::compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| panic!("{canary_rel} should compile:\n{diagnostics:#?}"));
    report
        .publish_retained_native_artifact(&build_dir)
        .unwrap_or_else(|error| panic!("{canary_rel} should publish: {error}"));

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
    let canary = repo_root().join("tests/omega/pass").join(canary_rel);
    for target in ["windows_x86_64", "linux_arm64"] {
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

        let staged_root = source_dir.join("main.omg");
        let package_inputs = reviewed_fixture_package_inputs(&staged_root, Some(target))
            .unwrap_or_else(|diagnostics| {
                panic!("{canary_rel} package inputs for {target}:\n{diagnostics:#?}")
            });
        let mut request = compiler::CompileRequest::new(CompileOptions {
            root_path: staged_root,
            build_dir: Some(build_dir),
            target_name: Some(target.to_owned()),
        })
        .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact);
        if let Some(package_inputs) = package_inputs {
            request = request.with_package_inputs(package_inputs);
        }
        compiler::compile(request)
            .and_then(compiler::CompileOutcomes::into_single_report)
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
        .join("tests/omega/fail")
        .join(canary_rel)
        .join("main.omg");
    compile_to_checked(CheckedCompileRequest::new(&canary, None))
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
            fixture_roster::RUNTIME_SCALAR_PUN_SHARED_LET_EXIT,
            "scalar-pun-shared",
        ),
        (
            fixture_roster::RUNTIME_INTERIOR_BYTE_RECAST_EXIT,
            "interior-byte-recast",
        ),
        (
            fixture_roster::RUNTIME_OFFSET_BYTE_RECAST_EXIT,
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
            fixture_roster::RUNTIME_SCALAR_PUN_MUTABLE_WRITE_EXIT,
            "mutable-scalar-pun",
        ),
        (
            fixture_roster::RUNTIME_OFFSET_BYTE_RECAST_MUTABLE_WRITE_EXIT,
            "mutable-byte-region",
        ),
    ] {
        assert_exit_70(canary, tag);
    }
}

#[test]
fn mutable_recasts_cross_compile() {
    compile_for_cross_targets(
        fixture_roster::RUNTIME_SCALAR_PUN_MUTABLE_WRITE_EXIT,
        "mutable-scalar-pun",
    );
    compile_for_cross_targets(
        fixture_roster::RUNTIME_OFFSET_BYTE_RECAST_MUTABLE_WRITE_EXIT,
        "mutable-byte-region",
    );
}

#[test]
fn flow_proven_recast_execution_canaries_run() {
    for (canary, tag) in [
        (
            fixture_roster::RUNTIME_MULTI_EDGE_OFFSET_MEET_EXIT,
            "multi-edge-offset-meet",
        ),
        (
            fixture_roster::RUNTIME_GUARDED_OFFSET_RECAST_EXIT,
            "guarded-offset-recast",
        ),
        (
            fixture_roster::RUNTIME_SYMBOLIC_STRIDE_FOOTPRINT_EXIT,
            "symbolic-stride-footprint",
        ),
    ] {
        assert_exit_70(canary, tag);
    }
}

#[test]
fn record_recast_execution_canaries_run() {
    for (canary, tag) in [
        (fixture_roster::RUNTIME_RECORD_VIEW_EXIT, "record-view"),
        (
            fixture_roster::RUNTIME_RECORD_ARRAY_VIEW_MUTABLE_WRITE_EXIT,
            "record-array-mutable-view",
        ),
        (
            fixture_roster::CONSTANT_OFFSET_RECORD_VIEW_AFTER_WRITE_EXIT,
            "constant-offset-record-view",
        ),
    ] {
        assert_exit_70(canary, tag);
    }

    let array_canary = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::RUNTIME_RECORD_ARRAY_VIEW_MUTABLE_WRITE_EXIT);
    let checked = compile_pass_to_checked(&array_canary.join("main.omg"));
    assert_eq!(
        interpret(&checked, &[]).exit_code,
        70,
        "the interpreter must preserve nested array/record offsets"
    );
    compile_for_cross_targets(
        fixture_roster::RUNTIME_RECORD_ARRAY_VIEW_MUTABLE_WRITE_EXIT,
        "record-array-mutable-view",
    );
}

#[test]
fn fixed_array_recast_execution_and_fact_fence() {
    let canary = fixture_roster::RUNTIME_FIXED_ARRAY_VIEW_MUTABLE_WRITE_EXIT;
    assert_exit_70(canary, "fixed-array-mutable-view");

    let main = repo_root()
        .join("tests/omega/pass")
        .join(canary)
        .join("main.omg");
    let checked = compile_pass_to_checked(&main);
    assert_eq!(
        interpret(&checked, &[]).exit_code,
        70,
        "the interpreter must preserve top-level fixed-array view identity"
    );

    compile_for_cross_targets(canary, "fixed-array-mutable-view");

    let diagnostics = fail_diagnostics(fixture_roster::FIXED_ARRAY_VIEW_FACT_FENCED);
    assert!(
        diagnostics.contains("must be recursively fact-free"),
        "raw bytes must not establish fixed-array element facts:\n{diagnostics}"
    );
}

#[test]
fn slice_recast_execution_tiling_and_fact_fences() {
    let canary = fixture_roster::RUNTIME_SLICE_VIEW_MUTABLE_WRITE_EXIT;
    assert_exit_70(canary, "slice-mutable-view");

    let main = repo_root()
        .join("tests/omega/pass")
        .join(canary)
        .join("main.omg");
    let checked = compile_pass_to_checked(&main);
    let interpreted = interpret(&checked, &[]);
    assert_eq!(
        interpreted.exit_code, 70,
        "the interpreter must derive slice length and preserve write-through: {interpreted:?}"
    );

    compile_for_cross_targets(canary, "slice-mutable-view");

    let non_tiling = fail_diagnostics(fixture_roster::SLICE_VIEW_NON_TILING_REJECTED);
    assert!(
        non_tiling.contains("does not exactly tile"),
        "non-divisible slice recast produced the wrong diagnostic:\n{non_tiling}"
    );
    let facted = fail_diagnostics(fixture_roster::SLICE_VIEW_FACT_FENCED);
    assert!(
        facted.contains("raw storage cannot establish element facts"),
        "raw bytes must not establish slice element facts:\n{facted}"
    );
}

#[test]
fn interior_slice_recasts_preserve_dynamic_tail_geometry() {
    let canary = fixture_roster::RUNTIME_INTERIOR_SLICE_VIEW_MUTABLE_WRITE_EXIT;
    assert_exit_70(canary, "interior-slice-mutable-view");

    let main = repo_root()
        .join("tests/omega/pass")
        .join(canary)
        .join("main.omg");
    let checked = compile_pass_to_checked(&main);
    let interpreted = interpret(&checked, &[]);
    assert_eq!(
        interpreted.exit_code, 70,
        "the interpreter must preserve interior slice length and write-through: {interpreted:?}"
    );

    compile_for_cross_targets(canary, "interior-slice-mutable-view");

    let non_tiling = fail_diagnostics(fixture_roster::INTERIOR_SLICE_RUNTIME_OFFSET_NON_TILING);
    assert!(
        non_tiling.contains("cannot prove exact tiling for interior slice"),
        "runtime-offset tiling produced the wrong diagnostic:\n{non_tiling}"
    );
    let facted = fail_diagnostics(fixture_roster::INTERIOR_SLICE_FACT_FENCED);
    assert!(
        facted.contains("raw storage cannot establish element facts"),
        "interior raw bytes must not establish slice element facts:\n{facted}"
    );
}

#[test]
fn interior_slice_congruent_runtime_offset_tiles() {
    let canary = fixture_roster::RUNTIME_INTERIOR_SLICE_CONGRUENT_OFFSET_EXIT;
    assert_exit_70(canary, "interior-slice-congruent-offset");

    let main = repo_root()
        .join("tests/omega/pass")
        .join(canary)
        .join("main.omg");
    let checked = compile_pass_to_checked(&main);
    let interpreted = interpret(&checked, &[]);
    assert_eq!(
        interpreted.exit_code, 70,
        "the interpreter must preserve the congruent-offset slice length and write-through: \
         {interpreted:?}"
    );

    compile_for_cross_targets(canary, "interior-slice-congruent-offset");
}

#[test]
fn aggregate_slice_recasts_compose_leaf_representation_sets() {
    let canary = fixture_roster::RUNTIME_AGGREGATE_SLICE_REPRESENTATION_RECAST_EXIT;
    assert_exit_70(canary, "aggregate-slice-representation-recast");

    let main = repo_root()
        .join("tests/omega/pass")
        .join(canary)
        .join("main.omg");
    let checked = compile_pass_to_checked(&main);
    let interpreted = interpret(&checked, &[]);
    assert_eq!(
        interpreted.exit_code, 70,
        "the interpreter must preserve aggregate slice facts and write-through: {interpreted:?}"
    );

    compile_for_cross_targets(canary, "aggregate-slice-representation-recast");

    let diagnostics = fail_diagnostics(fixture_roster::AGGREGATE_SLICE_MUT_LEAF_SETS_DIFFER);
    assert!(
        diagnostics.contains("fact implication in BOTH directions"),
        "aggregate slice leaf-set mismatch produced the wrong diagnostic:\n{diagnostics}"
    );
}

#[test]
fn mutable_recast_fact_fences_reject() {
    for canary in [
        fixture_roster::RECAST_MUT_FACT_FENCED,
        fixture_roster::RECAST_MUT_INTERIOR_FACT_FENCED,
        fixture_roster::RECAST_MUT_RECORD_FACT_FENCED,
        fixture_roster::RECAST_MUT_RECORD_ARRAY_FACT_FENCED,
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
    let canary = fixture_roster::RUNTIME_MUTABLE_EQUIVALENT_DOMAIN_RECAST_EXIT;
    assert_exit_70(canary, "mutable-equivalent-domain-recast");
}

#[test]
fn mutable_recast_accepts_equal_integer_representation_sets() {
    let canary = fixture_roster::RUNTIME_MUTABLE_EQUIVALENT_RANGE_RECAST_EXIT;
    assert_exit_70(canary, "mutable-equivalent-range-recast");
    compile_for_cross_targets(canary, "mutable-equivalent-range-recast");
}

#[test]
fn scalar_bool_recasts_follow_representation_set_implication() {
    let canary = fixture_roster::RUNTIME_BOOL_REPRESENTATION_RECAST_EXIT;
    assert_exit_70(canary, "bool-representation-recast");
    compile_for_cross_targets(canary, "bool-representation-recast");

    for fail in [
        fixture_roster::RECAST_SHARED_BOOL_FACT_FENCED,
        fixture_roster::RECAST_SHARED_INTERIOR_FACT_FENCED,
    ] {
        let diagnostics = fail_diagnostics(fail);
        assert!(
            diagnostics.contains("may weaken established facts but cannot strengthen them"),
            "{fail} produced the wrong shared representation-set diagnostic:\n{diagnostics}"
        );
    }
    let diagnostics = fail_diagnostics(fixture_roster::RECAST_MUT_BOOL_BIT_SETS_DIFFER);
    assert!(
        diagnostics.contains("fact implication in BOTH directions"),
        "mutable bool/full-byte alias produced the wrong diagnostic:\n{diagnostics}"
    );
}

#[test]
fn shared_domain_recasts_require_one_way_implication() {
    let canary = fixture_roster::RUNTIME_SHARED_DOMAIN_WEAKENING_RECAST_EXIT;
    assert_exit_70(canary, "shared-domain-weakening-recast");
    compile_for_cross_targets(canary, "shared-domain-weakening-recast");

    let diagnostics = fail_diagnostics(fixture_roster::RECAST_SHARED_DOMAIN_STRENGTHENING_REJECTED);
    assert!(
        diagnostics.contains("may weaken established facts but cannot strengthen them"),
        "shared domain strengthening produced the wrong diagnostic:\n{diagnostics}"
    );
}

#[test]
fn float_range_recasts_require_same_carrier_interval_implication() {
    let canary = fixture_roster::RUNTIME_FLOAT_RANGE_REPRESENTATION_RECAST_EXIT;
    assert_exit_70(canary, "float-range-representation-recast");
    compile_for_cross_targets(canary, "float-range-representation-recast");

    let diagnostics =
        fail_diagnostics(fixture_roster::RECAST_SHARED_FLOAT_RANGE_STRENGTHENING_REJECTED);
    assert!(
        diagnostics.contains("may weaken established facts but cannot strengthen them"),
        "shared float-range strengthening produced the wrong diagnostic:\n{diagnostics}"
    );

    let diagnostics = fail_diagnostics(fixture_roster::RECAST_MUT_FLOAT_RANGE_FENCED);
    assert!(
        diagnostics
            .contains("source and target constraints are not proven representation-equivalent"),
        "cross-carrier mutable float range produced the wrong diagnostic:\n{diagnostics}"
    );
}

#[test]
fn record_recasts_compose_same_carrier_float_leaf_intervals() {
    let canary = fixture_roster::RUNTIME_SHARED_RECORD_FLOAT_RANGE_WEAKENING_EXIT;
    assert_exit_70(canary, "shared-record-float-range-weakening");
    compile_for_cross_targets(canary, "shared-record-float-range-weakening");

    for fail in [
        fixture_roster::RECAST_SHARED_RECORD_FLOAT_LEAF_STRENGTHENING_REJECTED,
        fixture_roster::RECAST_MUT_RECORD_FLOAT_LEAF_SETS_DIFFER,
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
    let canary = fixture_roster::RUNTIME_MUTABLE_EQUIVALENT_RECORD_RECAST_EXIT;
    assert_exit_70(canary, "mutable-equivalent-record-recast");
    compile_for_cross_targets(canary, "mutable-equivalent-record-recast");
}

#[test]
fn mutable_recast_rejects_equal_looking_cross_carrier_domains() {
    let diagnostics =
        fail_diagnostics(fixture_roster::RECAST_MUT_CROSS_CARRIER_DOMAIN_NOT_EQUIVALENT);
    assert!(
        diagnostics
            .contains("source and target constraints are not proven representation-equivalent"),
        "wrong cross-carrier recast diagnostic:\n{diagnostics}"
    );
}

#[test]
fn mutable_recast_rejects_different_range_bit_sets() {
    let diagnostics = fail_diagnostics(fixture_roster::RECAST_MUT_RANGE_BIT_SETS_DIFFER);
    assert!(
        diagnostics
            .contains("source and target constraints are not proven representation-equivalent"),
        "wrong range representation-set diagnostic:\n{diagnostics}"
    );
}

#[test]
fn mutable_recast_does_not_treat_float_ranges_as_bit_pattern_sets() {
    let diagnostics = fail_diagnostics(fixture_roster::RECAST_MUT_FLOAT_RANGE_FENCED);
    assert!(
        diagnostics
            .contains("source and target constraints are not proven representation-equivalent"),
        "wrong float-range representation diagnostic:\n{diagnostics}"
    );
}

#[test]
fn mutable_recast_rejects_different_record_leaf_sets() {
    let diagnostics = fail_diagnostics(fixture_roster::RECAST_MUT_RECORD_LEAF_SETS_DIFFER);
    assert!(
        diagnostics.contains("identical layout geometry")
            && diagnostics.contains("fact implication in BOTH directions"),
        "wrong record representation-set diagnostic:\n{diagnostics}"
    );
}
