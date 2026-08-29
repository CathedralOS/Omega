use crate::manifest::dependencies::edit::BUILD_FILE_NAME;
use crate::manifest::dependencies::edit::model::{
    BuildDependencyEditError, BuildDependencyEditPlan, BuildDependencyManualReason,
    BuildFileReplacement,
};
use crate::manifest::dependencies::edit::planning::{
    plan_addition_from_source, plan_dependency_addition, plan_replacement_from_source,
};
use crate::manifest::dependencies::edit::rendering::{
    canonical_dependency_statement, source_digest,
};
use crate::manifest::dependencies::read::{
    DependencyProjectionError, DependencySourceRequest, PackageSelection, extract_from_source,
};
use omega_package_source::AliasName;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

fn path(location: &str) -> DependencySourceRequest {
    DependencySourceRequest::Path {
        explicit_alias: None,
        location: location.to_owned(),
    }
}

fn git(repository: &str, revision: &str) -> DependencySourceRequest {
    DependencySourceRequest::Git {
        explicit_alias: None,
        repository: repository.to_owned(),
        revision: revision.to_owned(),
        selection: PackageSelection::Root,
    }
}

fn automatic(plan: BuildDependencyEditPlan) -> BuildFileReplacement {
    let BuildDependencyEditPlan::Automatic(replacement) = plan else {
        panic!("expected automatic edit: {plan:?}");
    };
    replacement
}

fn fixture_root() -> PathBuf {
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "omega-package-dependency-edit-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create fixture");
    root
}

fn application_build(statements: &str) -> String {
    format!(
        "machine build(builder: &mut Build) {{\n    builder.application(\"dependency-edit-probe\");\n{statements}}}\n"
    )
}

#[test]
fn adds_to_empty_canonical_build_without_mutating_input() {
    let source = application_build("");
    let replacement = automatic(
        plan_addition_from_source(PathBuf::from("build.omg"), source.clone(), &path("../math"))
            .expect("plan addition"),
    );

    assert_eq!(source, application_build(""));
    assert_eq!(
        extract_from_source(replacement.replacement_source()).expect("project replacement"),
        vec![path("../math")]
    );
    assert!(
        replacement
            .replacement_source()
            .contains("    builder.depend(Source::Path { location: \"../math\" });")
    );
}

#[test]
fn rejects_dependency_edits_without_an_explicit_project_role() {
    let source = "target windows_x64 { }\n".to_owned();
    assert!(matches!(
        plan_addition_from_source(PathBuf::from("build.omg"), source, &path("vendor")),
        Err(BuildDependencyEditError::InvalidBuild(
            DependencyProjectionError::BuildDeclaration(error)
        )) if matches!(*error, crate::manifest::roles::BuildDeclarationError::MissingBuildDeclaration)
    ));
}

#[test]
fn appends_after_existing_build_work_and_preserves_it() {
    let source = r#"machine build(builder: &mut Build) {
    builder.application("dependency-edit-probe");
    builder.target(Target::Host);
}
"#
    .to_owned();
    let replacement = automatic(
        plan_addition_from_source(PathBuf::from("build.omg"), source, &path("vendor"))
            .expect("plan addition"),
    );

    assert!(
        replacement
            .replacement_source()
            .contains("    builder.target(Target::Host);\n    builder.depend")
    );
}

#[test]
fn noncanonical_signature_yields_generated_manual_patch() {
    let source = "machine build(builder: &mut Build, profile: u32) {\n    builder.application(\"dependency-edit-probe\");\n}\n".to_owned();
    let plan = plan_addition_from_source(PathBuf::from("build.omg"), source, &path("vendor"))
        .expect("plan addition");
    let BuildDependencyEditPlan::Manual(patch) = plan else {
        panic!("expected manual patch");
    };

    assert_eq!(
        patch.reason(),
        BuildDependencyManualReason::NonCanonicalBuildSignature
    );
    assert_eq!(
        patch.proposed_statement(),
        "builder.depend(Source::Path { location: \"vendor\" });"
    );
}

#[test]
fn replaces_a_semantically_canonical_row_without_relying_on_formatting() {
    let accepted = git("https://example.test/repo.git", "old");
    let candidate = git("https://example.test/repo.git", "new");
    let source = r#"machine build(builder: &mut Build) {
    builder.application("dependency-edit-probe");
    builder.depend(
        Source::Git {
            revision: "old",
            repository: "https://example.test/repo.git"
        }
    );
}
"#
    .to_owned();
    let replacement = automatic(
        plan_replacement_from_source(PathBuf::from("build.omg"), source, &accepted, &candidate)
            .expect("plan replacement"),
    );

    assert_eq!(
        extract_from_source(replacement.replacement_source()).expect("project replacement"),
        vec![candidate]
    );
}

#[test]
fn comments_inside_a_replaced_row_force_manual_placement() {
    let accepted = path("vendor");
    let candidate = path("vendor-next");
    let source = r#"machine build(builder: &mut Build) {
    builder.application("dependency-edit-probe");
    builder.depend(/* retained intent */ Source::Path { location: "vendor" });
}
"#
    .to_owned();
    let plan =
        plan_replacement_from_source(PathBuf::from("build.omg"), source, &accepted, &candidate)
            .expect("plan replacement");
    let BuildDependencyEditPlan::Manual(patch) = plan else {
        panic!("expected manual patch");
    };

    assert_eq!(
        patch.reason(),
        BuildDependencyManualReason::DependencyRowContainsComment
    );
}

#[test]
fn generated_rows_escape_all_caller_controlled_strings() {
    let request = DependencySourceRequest::Git {
        explicit_alias: Some(AliasName::parse("safe_alias").expect("alias")),
        repository: "https://example.test/\"repo\n// injected".to_owned(),
        revision: "main\rnext".to_owned(),
        selection: PackageSelection::Named(
            omega_package_source::PackageName::parse("selected-package").unwrap(),
        ),
    };
    let statement = canonical_dependency_statement(&request);
    let source = application_build(&format!("    {statement}\n"));

    assert!(!statement.contains("\n// injected"));
    assert_eq!(
        extract_from_source(&source).expect("escaped statement parses"),
        vec![request]
    );
}

#[test]
fn exact_existing_request_is_unchanged() {
    let request = path("vendor");
    let source = application_build(&format!(
        "    {}\n",
        canonical_dependency_statement(&request)
    ));

    assert_eq!(
        plan_addition_from_source(PathBuf::from("build.omg"), source, &request)
            .expect("plan addition"),
        BuildDependencyEditPlan::Unchanged
    );
}

#[test]
fn public_file_planner_binds_the_expected_digest_without_writing() {
    let root = fixture_root();
    let build_path = root.join(BUILD_FILE_NAME);
    let source = application_build("");
    fs::write(&build_path, &source).expect("write fixture build");

    let replacement =
        automatic(plan_dependency_addition(&root, &path("vendor")).expect("plan file addition"));

    assert_eq!(
        fs::read_to_string(&build_path).expect("read fixture"),
        source
    );
    assert_eq!(replacement.build_path(), build_path);
    assert_eq!(replacement.expected_sha256(), &source_digest(&source));
    fs::remove_dir_all(root).expect("remove fixture");
}
