use super::*;
use crate::operations::{
    CompilePreparedLocalProjectNativeError, PreparedLocalProjectNativeRequest,
    compile_prepared_local_project_for_native, prepare_local_project_for_target,
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
        prepare_local_project_for_target(&self.0.join(entry), TARGET)
            .expect("prepare exact project and target")
            .expect("project has build.omg")
    }

    fn request(&self, entry: &str, output: &str) -> PreparedLocalProjectCheckRequest {
        PreparedLocalProjectCheckRequest::new(self.prepare(entry), self.0.join(output), TARGET)
            .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly)
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
fn requested_entry_is_checked_and_reported_instead_of_main() {
    let project = Project::new();
    project.write(
        "source/build.omg",
        "machine build(builder: &mut Build) { builder.package(\"alternate-entry\"); }\n",
    );
    project.write("source/main.omg", "this is deliberately invalid Omega\n");
    project.write("source/entry.omg", "pub machine value() -> u64 { 7 }\n");
    let prepared = project.prepare("source/entry.omg");
    let report = check_prepared_local_project(
        PreparedLocalProjectCheckRequest::new(prepared, project.0.join("output"), TARGET)
            .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
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
fn package_check_does_not_relax_native_application_gate() {
    let project = Project::new();
    project.write(
        "source/build.omg",
        "machine build(builder: &mut Build) { builder.package(\"checked-library\"); }\n",
    );
    project.write("source/main.omg", "pub machine value() -> u64 { 7 }\n");
    let report = check_prepared_local_project(project.request("source/main.omg", "checked"))
        .expect("package roots support CHECK");
    assert_check_only(&report);
    let result = compile_prepared_local_project_for_native(PreparedLocalProjectNativeRequest::new(
        project.prepare("source/main.omg"),
        project.0.join("native"),
        TARGET,
    ));
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
fn application_check_settles_explicit_trust_without_native_root_policy() {
    let project = Project::new();
    project.write(
        "source/build.omg",
        r#"
machine build(builder: &mut Build) {
    builder.application("checked-trust");
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
