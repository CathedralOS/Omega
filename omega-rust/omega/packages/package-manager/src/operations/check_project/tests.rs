use super::{
    CheckPreparedLocalProjectError, CompileOutputKind, CompileReport,
    CompileResolvedPackageReviewsError, PathBuf, PreparedLocalProject,
    PreparedLocalProjectCheckRequest, TargetProfile, check_prepared_local_project,
};
use crate::operations::LocalProjectPreparationOptions;
use crate::operations::{
    CompilePreparedLocalProjectNativeError, PreparedLocalProjectNativeRequest,
    compile_prepared_local_project_for_native, prepare_local_project,
};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

mod generated;
mod semantic;

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);
const TARGET: TargetProfile = TargetProfile::WindowsX64;

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-prepared-check-{}-{}",
            std::process::id(),
            NEXT_PROJECT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn write(&self, relative: &str, source: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }

    fn prepare(&self, entry: &str) -> PreparedLocalProject {
        prepare_local_project(
            &self.0.join(entry),
            LocalProjectPreparationOptions {
                target: TARGET,
                offline: false,
            },
        )
        .expect("prepare exact project and target")
        .expect("project has build.omg")
    }

    fn request(&self, entry: &str, output: &str) -> PreparedLocalProjectCheckRequest {
        PreparedLocalProjectCheckRequest::new(self.prepare(entry), self.0.join(output), TARGET)
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_check_only(report: &CompileReport) {
    assert_eq!(report.output_kind(), CompileOutputKind::CheckOnly);
    assert!(!report.wrote_output());
    assert!(report.artifact().is_none());
    assert!(report.retained_native_artifact().is_none());
    assert!(report.production_manifest().is_none());
    assert!(report.executable_publication().is_none());
}

fn assert_empty_directory(path: &Path) {
    assert!(fs::read_dir(path).unwrap().next().is_none());
}

#[test]
fn checked_artifact_only_reports_complete_without_artifact_lowering() {
    let project = Project::new();
    project.write("source/main.omg", "pub machine value() -> u64 { 7 }\n");
    project.write(
        "source/build.omg",
        r#"
machine build(builder: &mut Build) {
    Build::application(builder, "report");
    Build::artifact_only(builder);
    let required: RequiredOutput = builder.output.require("result.txt");
    let path: BuildPath = builder.output.resolve("result.txt");
    let descriptor: i32 = builder.output.create(path, 438);
    let written: i64 = builder.output.write(descriptor, "report");
    let closed: i32 = builder.output.close(descriptor);
    let completion: OutputCompletion = builder.output.complete(required, path);
}
"#,
    );
    let (entry, closure, _) = project
        .prepare("source/main.omg")
        .into_review_parts(TARGET)
        .unwrap();
    let (checked, _reviews) = crate::review::compile_resolved_package_candidate_for_check(
        &closure.for_exact_target(TARGET),
        &project.0.join("review"),
        &entry,
        None,
        true,
    )
    .expect("retain the checked Build and completed output custody");
    for requested in [
        compiler::RequestedCompileProduct::Check,
        compiler::RequestedCompileProduct::TerminalArtifact,
        compiler::RequestedCompileProduct::NativeArtifact,
    ] {
        let prepared = checked
            .prepare_product_report(
                entry.clone(),
                requested,
                &compiler::OptimizationRollback::default(),
                compiler::TrustAdmissionSettlement::default(),
            )
            .expect("admit the checked artifact-only Build");
        let compilation_products::CheckedProductReport::Complete(report) = prepared else {
            panic!("artifact-only Build must not request Terminal or native lowering");
        };
        assert_eq!(
            report.source_file_count,
            checked.sources().source_file_count()
        );
        assert_eq!(report.timings(), checked.timings().phases());
        assert!(!report.timings().is_empty());
        assert!(!report.wrote_output());
        if requested == compiler::RequestedCompileProduct::Check {
            assert_check_only(&report);
            assert!(report.build_outputs().is_none());
        } else {
            assert_eq!(report.output_kind(), CompileOutputKind::BuildArtifacts);
            assert_eq!(
                report.build_observation_identity(),
                checked.build().observation_identity()
            );
            assert!(report.has_consistent_executable_publication_custody());
            assert_eq!(
                report.build_outputs().unwrap().identity(),
                checked
                    .build()
                    .completed_outputs()
                    .unwrap()
                    .unwrap()
                    .identity()
            );
        }
    }
}

#[test]
fn the_prepared_check_route_carries_the_stage_ladder_only_when_asked() {
    let project = Project::new();
    project.write(
        "source/build.omg",
        "machine build(builder: &mut Build) { Build::package(builder, \"timed\"); }\n",
    );
    project.write("source/main.omg", "pub machine value() -> u64 { 7 }\n");

    let quiet = check_prepared_local_project(project.request("source/main.omg", "quiet"))
        .expect("checked without the stage ladder");
    assert!(
        quiet.timings().is_empty(),
        "the ladder measures nothing until a caller asks for it"
    );

    let timed = check_prepared_local_project(
        project
            .request("source/main.omg", "timed")
            .with_timings(true),
    )
    .expect("checked with the stage ladder enabled");
    assert!(
        !timed.timings().is_empty(),
        "with_timings must reach the compiler through the prepared-project route"
    );
}

#[test]
fn requested_entry_is_checked_and_reported_instead_of_main() {
    let project = Project::new();
    project.write(
        "source/build.omg",
        "machine build(builder: &mut Build) { Build::package(builder, \"alternate_entry\"); }\n",
    );
    project.write("source/main.omg", "this is deliberately invalid Omega\n");
    project.write("source/entry.omg", "pub machine value() -> u64 { 7 }\n");
    let prepared = project.prepare("source/entry.omg");
    let report = check_prepared_local_project(PreparedLocalProjectCheckRequest::new(
        prepared,
        project.0.join("output"),
        TARGET,
    ))
    .expect("only the requested root source is checked");
    assert_check_only(&report);
    assert_eq!(report.root_path().file_name().unwrap(), "entry.omg");
    assert_ne!(report.root_path(), project.0.join("source/entry.omg"));
    assert_empty_directory(&project.0.join("output"));

    project.write("source/main.omg", "pub machine value() -> u64 { 7 }\n");
    project.write("source/entry.omg", "this selected source must reject\n");
    assert!(matches!(
        check_prepared_local_project(project.request("source/entry.omg", "rejected")),
        Err(CheckPreparedLocalProjectError::Review(
            CompileResolvedPackageReviewsError::Compilation { .. }
        ))
    ));
    assert_empty_directory(&project.0.join("rejected"));
}

#[test]
fn check_rejects_proof_product_requests_it_cannot_publish() {
    // A check-only stop publishes no artifact pair, so a build that declared
    // either optional proof product must reject rather than report a success
    // that silently dropped the request — the same admission fence the
    // standalone compiler's `RequestedCompileProduct::Check` arm applies.
    for pcc_line in [
        "builder.pcc.psi = true;",
        "builder.pcc.native = true;",
        "builder.pcc.psi = true; builder.pcc.native = true;",
    ] {
        let project = Project::new();
        project.write(
            "source/build.omg",
            &format!(
                "machine build(builder: &mut Build) {{ Build::application(builder, \"pcc_check\"); {pcc_line} }}\n"
            ),
        );
        project.write("source/main.omg", "pub machine value() -> u64 { 7 }\n");
        let result = check_prepared_local_project(project.request("source/main.omg", "rejected"));
        let Err(CheckPreparedLocalProjectError::ProofProduct(diagnostics)) = result else {
            panic!("check-only stop must reject `{pcc_line}`: {result:?}");
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.to_string().contains("check-only stop")),
            "expected a check-stop conflict diagnostic for `{pcc_line}`: {diagnostics:?}"
        );
        assert_empty_directory(&project.0.join("rejected"));
    }

    let project = Project::new();
    project.write(
        "source/build.omg",
        "machine build(builder: &mut Build) { Build::application(builder, \"pcc_check\"); }\n",
    );
    project.write("source/main.omg", "pub machine value() -> u64 { 7 }\n");
    let report = check_prepared_local_project(project.request("source/main.omg", "checked"))
        .expect("a build without proof-product requests still checks");
    assert_check_only(&report);
    assert!(!report.pcc_requests().any());
}

#[test]
fn package_check_does_not_relax_native_application_gate() {
    let project = Project::new();
    project.write(
        "source/build.omg",
        "machine build(builder: &mut Build) { Build::package(builder, \"checked_library\"); }\n",
    );
    project.write("source/main.omg", "pub machine value() -> u64 { 7 }\n");
    let report = check_prepared_local_project(project.request("source/main.omg", "checked"))
        .expect("package roots support CHECK");
    assert_check_only(&report);
    let result = compile_prepared_local_project_for_native(
        PreparedLocalProjectNativeRequest::new(
            project.prepare("source/main.omg"),
            project.0.join("native"),
            TARGET,
        ),
        |_| (),
    )
    .map(|(report, ())| report);
    assert!(matches!(
        result,
        Err(CompilePreparedLocalProjectNativeError::Review(
            CompileResolvedPackageReviewsError::InvalidProductionRootRole {
                role: package_compilation::BuildDeclarationKind::Package,
                ..
            }
        ))
    ));
    assert!(!project.0.join("native").exists());
}

#[test]
fn package_check_rejects_individual_machine_grants_before_reporting() {
    let project = Project::new();
    project.write(
        "source/build.omg",
        r#"machine build(builder: &mut Build) {
    Build::application(builder, "machine_grant");
    builder.accept_boundary<Claim::accepted>();
}
"#,
    );
    project.write(
        "source/main.omg",
        "pub data Claim {}\nboundary machine Claim::accepted() ensures true;\n",
    );
    let error = check_prepared_local_project(project.request("source/main.omg", "rejected"))
        .expect_err("package checking must not substitute an individual grant for package review");
    assert!(
        error
            .to_string()
            .contains("cannot admit individual accepted machine"),
        "unexpected rejection: {error}"
    );
    assert_empty_directory(&project.0.join("rejected"));
    assert!(!project.0.join("source/omega.lock").exists());
}

#[test]
fn application_check_settles_explicit_trust_without_native_root_policy() {
    let project = Project::new();
    project.write(
        "source/build.omg",
        r#"
machine build(builder: &mut Build) {
    Build::application(builder, "checked_trust");
    builder.accept_boundary<ClaimProvider>();
    builder.select_provider<ClaimProvider, ClaimProviderImpl>();
}
"#,
    );
    project.write(
        "source/main.omg",
        r#"
boundary trait ClaimProvider { machine admitted() ensures true; }
data ClaimProviderImpl {}
machine ClaimProviderImpl::admitted() satisfies ClaimProvider::admitted {}
data Main {}
machine Main::exercise(&mut self) {}
"#,
    );
    let report = check_prepared_local_project(project.request("source/main.omg", "unresolved"))
        .expect("CHECK reports trust without requiring native policy admission");
    assert_check_only(&report);
    let settlement = report.trust_admission_settlement();
    assert_eq!(settlement.unresolved().len(), 1);
    assert_eq!(settlement.required(), settlement.unresolved());
    let report = check_prepared_local_project(
        project
            .request("source/main.omg", "admitted")
            .with_accepted_trust_admissions(settlement.required().to_vec()),
    )
    .expect("explicit checked trust admissions settle against the current root");
    assert!(report.trust_admission_settlement().is_exactly_admitted());
    assert_eq!(report.trust_admission_settlement().consumed().len(), 1);
    assert_check_only(&report);
    assert!(!project.0.join("source/omega.lock").exists());
}
