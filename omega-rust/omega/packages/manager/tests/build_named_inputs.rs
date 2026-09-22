//! Captured dependency slots feed generated source and ordinary accepted publication.
use crate::accepted_policy_fixture;

use package_compilation::{
    BuildDependencyOccurrence, BuildSourceCaptureObligation, BuildSourceCaptureRequest,
    capture_scoped_source_input,
};
use package_manager::admission::{
    AcceptedNativeInput, AcceptedNativeRealizationRequest, accept_ordinary_closure_evidence,
    realize_accepted_native_report,
};
use package_manager::resolution::graph::{
    PackageSourceClosureLimits, ResolvedPackageSourceClosure, resolve_workspace_project_closure,
};
use package_manager::review::{
    CanonicalPackageReconstructionQuestionLimits, ReviewOnlyCapabilityConflictLimits,
    SemanticBindingReview, compile_resolved_package_candidate_for_production,
};
use package_source::{
    LocalSourceLimits, PrimaryGitChoices, SourceLineage, SourceRelativePath, SourceResolverStorage,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-named-inputs-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn write(&self, relative: &str, bytes: impl AsRef<[u8]>) {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
fn seal_input(root: &Path, sealed: bool) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(
        root.join("source.omg"),
        std::fs::Permissions::from_mode(if sealed { 0o444 } else { 0o644 }),
    )
    .unwrap();
    std::fs::set_permissions(
        root,
        std::fs::Permissions::from_mode(if sealed { 0o555 } else { 0o755 }),
    )
    .unwrap();
}
#[cfg(not(unix))]
fn seal_input(_root: &Path, _sealed: bool) {}

const ROOT_BUILD: &str = r#"use first::main;
use second::main;
machine build(builder: &mut Build) {
    builder.application("named-input-consumer");
    builder.artifact_only();
    builder.build_depend_as("first", Source::Path { location: "../first" });
    builder.build_depend_as("second", Source::Path { location: "../second" });
    let left: u8 = first_byte();
    let right: u8 = second_byte();
    let bytes: [u8; 2] = [left, right];
    let required: RequiredOutput = builder.output.require("result.txt");
    let path: BuildPath = builder.output.resolve("result.txt");
    let descriptor: i32 = builder.output.create(path, 438);
    let written: i64 = builder.output.write(descriptor, &bytes);
    let closed: i32 = builder.output.close(descriptor);
    let completion: OutputCompletion = builder.output.complete(required, path);
}
"#;

fn prepare(
    invalid_source: bool,
    omit_second: bool,
) -> (
    Fixture,
    ResolvedPackageSourceClosure,
    build_evaluation::BuildSnapshotRequest,
) {
    let fixture = Fixture::new();
    fixture.write("workspace/consumer/main.omg", "data Main {}\n");
    fixture.write("workspace/consumer/build.omg", ROOT_BUILD);
    for (name, function, value) in [("first", "first_byte", 65), ("second", "second_byte", 66)] {
        let source = if invalid_source && name == "second" {
            "not valid Omega source!".to_owned()
        } else {
            format!("pub machine {function}() -> u8 {{ {value} }}\n")
        };
        fixture.write(&format!("inputs/{name}/source.omg"), &source);
        fixture.write(
            &format!("workspace/{name}/main.omg"),
            format!("pub data {name}Marker {{}}\n"),
        );
        fixture.write(
            &format!("workspace/{name}/build.omg"),
            format!(
                r#"machine build(builder: &mut Build) {{
    builder.package("{name}-generator");
    let input: BuildSource = builder.inputs.get("template");
    let path: BuildPath = input.resolve("source.omg");
    let descriptor: i32 = input.open(path, 0);
    let mut bytes: [u8; {length}];
    let count: i64 = input.read(descriptor, &mut bytes, {length});
    let closed: i32 = input.close(descriptor);
    let generated: BuildPath = builder.output.resolve("value.generated.omg");
    let output: i32 = builder.output.create(generated, 438);
    let written: i64 = builder.output.write(output, &bytes);
    let released: i32 = builder.output.close(output);
    builder.output.include_source(generated);
}}
"#,
                length = source.len()
            ),
        );
    }
    let storage = SourceResolverStorage::for_hardened_base(
        fixture.0.join("cache"),
        PrimaryGitChoices::default(),
    )
    .unwrap();
    let closure = resolve_workspace_project_closure(
        &SourceLineage::git("https://example.com/named-input-fixture.git").unwrap(),
        SourceRelativePath::parse("consumer").unwrap(),
        fixture.0.join("workspace"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .unwrap();
    let graph = closure.graph();
    let mut assignments = Vec::new();
    for edge in graph.package(graph.root()).unwrap().dependencies() {
        let name = edge.alias().as_str();
        if omit_second && name == "second" {
            continue;
        }
        let root = fixture.0.join("inputs").join(name);
        seal_input(&root, true);
        let captured = capture_scoped_source_input(
            &root,
            &BuildSourceCaptureRequest::new([(
                b"source.omg".to_vec(),
                BuildSourceCaptureObligation::Required,
            )])
            .unwrap(),
            Vec::<PathBuf>::new(),
        )
        .unwrap();
        seal_input(&root, false);
        // The evaluator must consume captured bytes, never reopen this changed host file.
        std::fs::write(root.join("source.omg"), "host changed after capture").unwrap();
        assignments.push((
            BuildDependencyOccurrence::new(
                graph.root().identity(),
                edge.purpose(),
                name,
                edge.target().identity(),
            ),
            b"template".to_vec(),
            captured,
        ));
    }
    let snapshot = build_evaluation::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>())
        .with_dependency_inputs(assignments)
        .unwrap();
    (fixture, closure, snapshot)
}

#[test]
fn named_dependency_inputs_reach_generated_source_and_completed_publication() {
    let (fixture, closure, snapshot) = prepare(false, false);
    let target = closure.for_exact_target(target::TargetProfile::LinuxX64);
    let candidate = compile_resolved_package_candidate_for_production(
        &target,
        &fixture.0.join("review"),
        SemanticBindingReview::Explicit(&[]),
        Some(&snapshot),
    )
    .expect("captured slots feed acquired dependency builds");
    for (package, function, value) in [
        ("first-generator", "first_byte", 65),
        ("second-generator", "second_byte", 66),
    ] {
        let review = candidate
            .reviews()
            .reviews()
            .iter()
            .find(|review| review.key().name().as_str() == package)
            .unwrap();
        let [generated] = review.generated_source_bundle().sources() else {
            panic!("one generated source per dependency")
        };
        let observation = review.build_observation_summary().unwrap();
        let [input] = observation.named_input_inventories() else {
            panic!("one isolated captured slot per dependency")
        };
        assert_eq!(input.name(), b"template");
        assert_eq!(input.root_identity(), 3);
        assert_eq!(input.inventory().entry_count(), 2);
        assert_eq!(
            input.inventory().file_bytes(),
            generated.bytes().len() as u64
        );
        let staged = observation.staged_output_tree().unwrap();
        assert_eq!(staged.entry_count(), 1);
        assert_eq!(staged.file_bytes(), generated.bytes().len() as u64);
        assert_eq!(generated.relative_path(), b"value.generated.omg");
        assert_eq!(
            generated.bytes(),
            format!("pub machine {function}() -> u8 {{ {value} }}\n").as_bytes()
        );
    }
    let policy = accepted_policy_fixture::accepted_policy(&target, candidate.reviews());
    let evidence = accept_ordinary_closure_evidence(
        &target,
        candidate.reviews(),
        CanonicalPackageReconstructionQuestionLimits::default(),
        ReviewOnlyCapabilityConflictLimits::default(),
        Some(&policy),
    )
    .unwrap();
    let report = realize_accepted_native_report(
        AcceptedNativeInput::Reviewed {
            candidate: Box::new(candidate),
            optimization_rollback: &compiler::OptimizationRollback::default(),
        },
        AcceptedNativeRealizationRequest {
            evidence: &evidence,
            profile: &proof_admission::AdmissionProfile::default(),
            terminal_authority_policy: native_realization::current_terminal_authority_policy(),
            receiving_terminal_authority_permission_policy: None,
            imports: &[],
        },
    )
    .expect("ordinary accepted artifact-only production");
    let publication = fixture.0.join("published");
    assert!(!publication.exists());
    assert_eq!(
        report.output_kind(),
        compiler::CompileOutputKind::BuildArtifacts
    );
    let report = report
        .publish_completed_build_outputs(&publication)
        .unwrap();
    let outputs = report.build_outputs().unwrap();
    let directory = outputs.published_directory().unwrap();
    assert_eq!(
        std::fs::read(directory.join("files/result.txt")).unwrap(),
        b"AB"
    );
    assert_eq!(
        std::fs::read(directory.join("manifest.bin")).unwrap(),
        outputs.manifest_bytes()
    );
    assert_eq!(
        std::fs::read_dir(directory.join("files")).unwrap().count(),
        1
    );
    assert!(report.has_consistent_executable_publication_custody());
}

#[test]
fn absent_named_dependency_input_rejects_before_publication() {
    let (fixture, closure, snapshot) = prepare(false, true);
    let error = compile_resolved_package_candidate_for_production(
        &closure.for_exact_target(target::TargetProfile::LinuxX64),
        &fixture.0.join("review"),
        SemanticBindingReview::Explicit(&[]),
        Some(&snapshot),
    )
    .expect_err("missing slot cannot borrow another dependency's input");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("named build input") && diagnostic.contains("not assigned"),
        "{diagnostic}"
    );
    assert!(!fixture.0.join("published").exists());
}

#[test]
fn invalid_generated_named_input_cannot_publish_a_successful_product() {
    let (fixture, closure, snapshot) = prepare(true, false);
    let error = compile_resolved_package_candidate_for_production(
        &closure.for_exact_target(target::TargetProfile::LinuxX64),
        &fixture.0.join("review"),
        SemanticBindingReview::Explicit(&[]),
        Some(&snapshot),
    )
    .expect_err("invalid captured source must fail ordinary generated-source checking");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("value.generated.omg"), "{diagnostic}");
    assert!(!fixture.0.join("published").exists());
}

#[test]
fn shared_dependency_activation_requires_equal_complete_input_assignments() {
    for assignment in ["equal", "different", "unassigned"] {
        let fixture = Fixture::new();
        fixture.write("workspace/consumer/main.omg", "data Main {}\n");
        fixture.write(
            "workspace/consumer/build.omg",
            r#"machine build(builder: &mut Build) {
    builder.application("shared-input-consumer");
    builder.artifact_only();
    builder.build_depend_as("first", Source::Path { location: "../shared" });
    builder.build_depend_as("second", Source::Path { location: "../shared" });
    let required: RequiredOutput = builder.output.require("result.txt");
    let path: BuildPath = builder.output.resolve("result.txt");
    let descriptor: i32 = builder.output.create(path, 438);
    let written: i64 = builder.output.write(descriptor, "shared");
    let closed: i32 = builder.output.close(descriptor);
    let completion: OutputCompletion = builder.output.complete(required, path);
}
"#,
        );
        fixture.write("workspace/shared/main.omg", "pub data Marker {}\n");
        fixture.write(
            "workspace/shared/build.omg",
            r#"machine build(builder: &mut Build) {
    builder.package("shared-generator");
    let input: BuildSource = builder.inputs.get("template");
}
"#,
        );
        let storage = SourceResolverStorage::for_hardened_base(
            fixture.0.join("cache"),
            PrimaryGitChoices::default(),
        )
        .unwrap();
        let closure = resolve_workspace_project_closure(
            &SourceLineage::git("https://example.com/shared-input-fixture.git").unwrap(),
            SourceRelativePath::parse("consumer").unwrap(),
            fixture.0.join("workspace"),
            &storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .unwrap();
        let graph = closure.graph();
        let mut assignments = Vec::new();
        for edge in graph.package(graph.root()).unwrap().dependencies() {
            let name = edge.alias().as_str();
            if name == "second" && assignment == "unassigned" {
                continue;
            }
            fixture.write(
                &format!("inputs/{name}/source.omg"),
                if name == "second" && assignment == "different" {
                    "different"
                } else {
                    "same"
                },
            );
            let root = fixture.0.join("inputs").join(name);
            seal_input(&root, true);
            let captured = capture_scoped_source_input(
                &root,
                &BuildSourceCaptureRequest::new([(
                    b"source.omg".to_vec(),
                    BuildSourceCaptureObligation::Required,
                )])
                .unwrap(),
                Vec::<PathBuf>::new(),
            )
            .unwrap();
            seal_input(&root, false);
            assignments.push((
                BuildDependencyOccurrence::new(
                    graph.root().identity(),
                    edge.purpose(),
                    name,
                    edge.target().identity(),
                ),
                b"template".to_vec(),
                captured,
            ));
        }
        let snapshot = build_evaluation::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>())
            .with_dependency_inputs(assignments)
            .unwrap();
        let result = compile_resolved_package_candidate_for_production(
            &closure.for_exact_target(target::TargetProfile::LinuxX64),
            &fixture.0.join("review"),
            SemanticBindingReview::Explicit(&[]),
            Some(&snapshot),
        );
        if assignment == "equal" {
            let candidate = result.expect("equal complete maps may share one checked activation");
            assert_eq!(
                candidate
                    .reviews()
                    .reviews()
                    .iter()
                    .filter(|review| review.key().name().as_str() == "shared-generator")
                    .count(),
                1
            );
        } else {
            let error =
                result.expect_err("incompatible incoming assignments must reject before execution");
            let diagnostic = format!("{error:?}");
            assert!(
                diagnostic.contains("conflicting named build input assignments"),
                "{assignment}: {diagnostic}"
            );
            assert!(
                !diagnostic.contains("not assigned"),
                "the getter must not run: {diagnostic}"
            );
            assert!(!fixture.0.join("published").exists());
        }
    }
}
