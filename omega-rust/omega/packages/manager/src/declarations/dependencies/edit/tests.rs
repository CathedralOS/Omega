use crate::declarations::dependencies::edit::BUILD_FILE_NAME;
use crate::declarations::dependencies::edit::model::{
    BuildDependencyEditError, BuildDependencyEditPlan, BuildDependencyManualReason,
    BuildFileReplacement,
};
use crate::declarations::dependencies::edit::planning::plan_dependency_addition;
use crate::declarations::dependencies::edit::rendering::{
    canonical_dependency_statement, source_digest,
};
use crate::declarations::dependencies::read::{
    DependencyProjectionError, DependencySourceRequest, PackageSelection, extract_from_source,
};
use crate::declarations::{
    AliasName, plan_dependency_addition_from_source, plan_dependency_replacement_from_source,
};
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
        plan_dependency_addition_from_source(
            PathBuf::from("build.omg"),
            source.clone(),
            &path("../math"),
        )
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
    let source = "".to_owned();
    assert!(matches!(
        plan_dependency_addition_from_source(PathBuf::from("build.omg"), source, &path("vendor")),
        Err(BuildDependencyEditError::InvalidBuild(
            DependencyProjectionError::BuildDeclaration(error)
        )) if matches!(*error, crate::declarations::roles::BuildDeclarationError::MissingBuildDeclaration)
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
        plan_dependency_addition_from_source(PathBuf::from("build.omg"), source, &path("vendor"))
            .expect("plan addition"),
    );

    assert!(
        replacement
            .replacement_source()
            .contains("    builder.target(Target::Host);\n    builder.depend")
    );
}

#[test]
fn noncanonical_signature_rejects_before_edit_planning() {
    let source = "machine build(builder: &mut Build, profile: u32) {\n    builder.application(\"dependency-edit-probe\");\n}\n".to_owned();
    assert!(matches!(
        plan_dependency_addition_from_source(PathBuf::from("build.omg"), source, &path("vendor")),
        Err(BuildDependencyEditError::InvalidBuild(
            DependencyProjectionError::InvalidBuildParameter
        ))
    ));
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
        plan_dependency_replacement_from_source(
            PathBuf::from("build.omg"),
            source,
            &accepted,
            &candidate,
        )
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
    let plan = plan_dependency_replacement_from_source(
        PathBuf::from("build.omg"),
        source,
        &accepted,
        &candidate,
    )
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
            crate::declarations::PackageName::parse("selected-package").unwrap(),
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
        plan_dependency_addition_from_source(PathBuf::from("build.omg"), source, &request)
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

#[test]
fn replacement_from_unchanged_sources_retains_exact_bytes() {
    let source = "// retained context: café\r\nmachine build(builder: &mut Build) { builder.application(\"probe\"); }\r\n";
    let build_path = PathBuf::from("command-owned/build.omg");
    let replacement =
        BuildFileReplacement::from_sources(build_path.clone(), source, source.to_owned())
            .expect("unchanged source is a replacement");

    assert_eq!(replacement.build_path(), build_path);
    assert_eq!(
        replacement.replacement_source().as_bytes(),
        source.as_bytes()
    );
    assert_eq!(replacement.expected_sha256(), &source_digest(source));
}

#[test]
fn replacement_from_sources_recovers_a_saved_proposal() {
    let source = application_build("");
    let build_path = PathBuf::from("command-owned/build.omg");
    let planned = automatic(
        plan_dependency_addition_from_source(build_path.clone(), source.clone(), &path("vendor"))
            .expect("plan addition"),
    );
    let recovered = BuildFileReplacement::from_sources(
        build_path,
        &source,
        planned.replacement_source().to_owned(),
    )
    .expect("recover saved proposal");

    assert_eq!(recovered, planned);
}

#[test]
fn replacement_from_sources_rejects_invalid_before_and_proposed() {
    let valid = application_build("");
    let invalid_sources = [
        String::new(),
        "machine build(".to_owned(),
        application_build("    builder.depend(Source::Path { location: 42 });\n"),
    ];
    for invalid in invalid_sources {
        let expected = BuildDependencyEditError::InvalidBuild(
            extract_from_source(&invalid).expect_err("ordinary projection rejects source"),
        );
        assert_eq!(
            BuildFileReplacement::from_sources(PathBuf::from("build.omg"), &invalid, valid.clone(),),
            Err(expected.clone()),
            "invalid before must be rejected",
        );
        assert_eq!(
            BuildFileReplacement::from_sources(PathBuf::from("build.omg"), &valid, invalid),
            Err(expected),
            "invalid proposal must be rejected",
        );
    }
}

#[test]
fn replacement_from_sources_hashes_raw_before_context() {
    use sha2::{Digest, Sha256};

    let source = application_build("");
    let proposed =
        application_build("    builder.depend(Source::Path { location: \"vendor\" });\n");
    let variants = [
        source.clone(),
        source.replace('\n', "\r\n"),
        format!("// retained context: café\n{source}"),
    ];
    let mut digests = Vec::new();
    for before in variants {
        assert_eq!(
            extract_from_source(&before).expect("project before"),
            extract_from_source(&source).expect("project original"),
        );
        let replacement = BuildFileReplacement::from_sources(
            PathBuf::from("build.omg"),
            &before,
            proposed.clone(),
        )
        .expect("construct replacement");
        let expected: [u8; 32] = Sha256::digest(before.as_bytes()).into();
        assert_eq!(replacement.expected_sha256(), &expected);
        assert_ne!(replacement.expected_sha256(), &source_digest(&proposed));
        assert_eq!(replacement.replacement_source(), proposed);
        assert!(!digests.contains(&expected));
        digests.push(expected);
    }
}

#[test]
fn source_apis_use_command_bytes_when_the_file_differs() {
    let root = fixture_root();
    let build_path = root.join(BUILD_FILE_NAME);
    let disk_source = "different current contents";
    fs::write(&build_path, disk_source).expect("write fixture");
    let source = application_build("");
    let accepted = path("vendor");
    let candidate = path("vendor-next");
    let addition = automatic(
        plan_dependency_addition_from_source(build_path.clone(), source.clone(), &accepted)
            .expect("plan against command source"),
    );
    assert_eq!(addition.expected_sha256(), &source_digest(&source));
    let before = addition.replacement_source();
    let replacement = automatic(
        plan_dependency_replacement_from_source(
            build_path.clone(),
            before.to_owned(),
            &accepted,
            &candidate,
        )
        .expect("replace against command source"),
    );
    assert_eq!(replacement.expected_sha256(), &source_digest(before));
    assert_eq!(
        extract_from_source(replacement.replacement_source()).expect("project replacement"),
        vec![candidate],
    );
    assert_eq!(
        BuildFileReplacement::from_sources(
            build_path.clone(),
            before,
            replacement.replacement_source().to_owned(),
        )
        .expect("recover without reading current file"),
        replacement,
    );
    assert_eq!(
        fs::read_to_string(build_path).expect("read fixture"),
        disk_source
    );
    fs::remove_dir_all(root).expect("remove fixture");
}
