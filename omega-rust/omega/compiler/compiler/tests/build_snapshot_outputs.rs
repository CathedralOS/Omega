// Focused coverage for compiler-owned required-output obligations issued
// through `builder.output.require`/`complete`/`fail`, the `artifact_only`
// application modifier, and the executable-with-companion route. Each fixture
// runs the admitted build machine against a fresh sponsored output root bound
// to a captured source snapshot.

use checked_interpreter::FilesystemSponsor;
use compiler::CheckedCompileRequest;
use compiler::compile_to_checked;
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::path::{Path, PathBuf};

struct Project {
    root: PathBuf,
}

impl Project {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-snapshot-outputs-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).expect("create snapshot-output project");
        Self { root }
    }

    fn write(&self, relative: &str, source: &str) {
        std::fs::write(self.root.join(relative), source).expect("write snapshot-output fixture");
    }

    fn main(&self) -> PathBuf {
        self.root.join("main.omg")
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        set_canonical_source_tree_permissions(&self.root, false);
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[cfg(unix)]
fn set_canonical_source_tree_permissions(root: &Path, sealed: bool) {
    use std::os::unix::fs::PermissionsExt;

    let metadata = std::fs::symlink_metadata(root).expect("inspect package source fixture");
    if metadata.is_dir() {
        if !sealed {
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o755))
                .expect("unseal package source directory");
        }
        for entry in std::fs::read_dir(root).expect("enumerate package source fixture") {
            set_canonical_source_tree_permissions(
                &entry.expect("read package source entry").path(),
                sealed,
            );
        }
        if sealed {
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o555))
                .expect("seal package source directory");
        }
    } else {
        let mode = if sealed { 0o444 } else { 0o644 };
        std::fs::set_permissions(root, std::fs::Permissions::from_mode(mode))
            .expect("set package source file permissions");
    }
}

#[cfg(not(unix))]
fn set_canonical_source_tree_permissions(_root: &Path, _sealed: bool) {}

fn package_inputs(root: &Path) -> PackageCompilationInputs {
    let package = PackageKeyIdentity::from_digest([89; 32]).expect("nonzero package identity");
    PackageCompilationInputs::new_package(
        package,
        vec![
            PackageSourceBinding::new(package, "snapshot-outputs", root.to_path_buf())
                .with_canonical_source_metadata()
                .expect("capture canonical package source"),
        ],
        Vec::<PackageDependencyBinding>::new(),
    )
    .expect("single-package snapshot-output input")
}

fn bound_build_output_session(label: &str) -> (PathBuf, FilesystemSponsor, PathBuf) {
    let session = std::env::temp_dir().join(format!(
        "omega-snapshot-outputs-session-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&session);
    std::fs::create_dir(&session).expect("create snapshot-output session");
    let session = std::fs::canonicalize(session).expect("canonicalize snapshot-output session");
    let sponsor = FilesystemSponsor::new(&session).expect("create snapshot-output sponsor");
    let build_dir = session.join("output");
    let bound_build_dir = sponsor
        .bind_path(&build_dir)
        .expect("bind snapshot output root");
    let prepared_build_dir = sponsor
        .prepare_create_directory(&bound_build_dir)
        .expect("prepare snapshot output root");
    std::fs::create_dir(&build_dir).expect("create snapshot output root");
    prepared_build_dir
        .commit()
        .expect("commit snapshot output root");
    (session, sponsor, build_dir)
}

fn run_build(
    project: &Project,
    label: &str,
    target_name: &'static str,
    required_outputs: &[&[u8]],
) -> Result<compiler::CheckedCompilation, Vec<diagnostics::Diagnostic>> {
    let (session, sponsor, build_dir) = bound_build_output_session(label);
    set_canonical_source_tree_permissions(&project.root, true);
    let result = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(build_dir),
        package_inputs: Some(package_inputs(&project.root)),
        filesystem_sponsor: Some(sponsor),
        build_snapshot: Some(build_evaluation::BuildSnapshotRequest::new(
            required_outputs.iter().map(|name| name.to_vec()),
        )),
        ..CheckedCompileRequest::new(&project.main(), Some(target_name))
    });
    set_canonical_source_tree_permissions(&project.root, false);
    let _ = std::fs::remove_dir_all(session);
    result
}

fn diagnostic_messages(diagnostics: &[diagnostics::Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn required_output_completes_as_a_sealed_regular_file() {
    let project = Project::new("complete");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-output-complete");
    let required: RequiredOutput = builder.output.require("artifact.txt");
    let required_path: &[u8] = required.path();
    let artifact: BuildPath = builder.output.resolve(required_path);
    let descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(descriptor, "banner\n");
    let closed: i32 = builder.output.close(descriptor);
    let completion: OutputCompletion = builder.output.complete(required, artifact);
}
"#,
    );

    let checked =
        run_build(&project, "complete", "linux_x86_64", &[]).unwrap_or_else(|diagnostics| {
            panic!(
                "a completed required output must publish the build: {}",
                diagnostic_messages(&diagnostics)
            )
        });
    let observation = checked
        .build_observation_summary()
        .expect("required-output execution retains a build observation");
    let settlements = observation.required_output_settlements();
    assert_eq!(settlements.len(), 1, "exactly one obligation settled");
    assert_eq!(settlements[0].relative_path(), b"artifact.txt");
    let staged = observation
        .staged_output_tree()
        .expect("completed outputs remain in staged custody");
    let entry = staged
        .sealed_entry(b"artifact.txt")
        .expect("the completed required output is discoverable in sealed custody");
    let build_output::BuildStagedOutputEntryKind::File { bytes, .. } = entry.kind() else {
        panic!("a required output completes only as a sealed regular file")
    };
    assert_eq!(bytes, b"banner\n");
}

#[test]
fn required_output_completion_retries_an_unclosed_writer() {
    let project = Project::new("retry");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-output-retry");
    let required: RequiredOutput = builder.output.require("retry.txt");
    let required_path: &[u8] = required.path();
    let artifact: BuildPath = builder.output.resolve(required_path);
    let descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(descriptor, "retried\n");
    let first: OutputCompletion = builder.output.complete(required, artifact);
    transition first {
        OutputCompletion::Retry { obligation, file } -> finish(builder, obligation, file, descriptor)
        _ -> unexpected(builder)
    }

    state finish(
        builder: &mut Build,
        obligation: RequiredOutput,
        file: BuildPath,
        descriptor: i32
    ) {
        let closed: i32 = builder.output.close(descriptor);
        let second: OutputCompletion = builder.output.complete(obligation, file);
    }

    state unexpected(builder: &mut Build) {
        builder.log.write_line("unexpected completion verdict");
    }
}
"#,
    );

    let checked = run_build(&project, "retry", "linux_x86_64", &[]).unwrap_or_else(|diagnostics| {
        panic!(
            "a retried required-output completion must publish the build: {}",
            diagnostic_messages(&diagnostics)
        )
    });
    let observation = checked
        .build_observation_summary()
        .expect("retried completion retains a build observation");
    let settlements = observation.required_output_settlements();
    assert_eq!(settlements.len(), 1, "the retried obligation settles once");
    assert_eq!(settlements[0].relative_path(), b"retry.txt");
}

#[test]
fn required_output_declared_but_never_completed_rejects() {
    let project = Project::new("omitted");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-output-omitted");
    let required: RequiredOutput = builder.output.require("artifact.txt");
}
"#,
    );

    let diagnostics = run_build(&project, "omitted", "linux_x86_64", &[])
        .expect_err("an obligation the build never completed must reject publication");
    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains("required output `artifact.txt`")
            && messages.contains("declared but never completed"),
        "unexpected diagnostics: {messages}"
    );
}

#[test]
fn required_output_declared_failure_rejects() {
    let project = Project::new("failed");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-output-failed");
    let required: RequiredOutput = builder.output.require("artifact.txt");
    builder.output.fail(required, "generator unavailable");
}
"#,
    );

    let diagnostics = run_build(&project, "failed", "linux_x86_64", &[])
        .expect_err("a failed obligation must reject publication");
    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains("required output `artifact.txt`")
            && messages.contains("failed: generator unavailable"),
        "unexpected diagnostics: {messages}"
    );
}

#[test]
fn duplicate_required_output_name_rejects() {
    let project = Project::new("duplicate");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-output-duplicate");
    let first: RequiredOutput = builder.output.require("artifact.txt");
    let second: RequiredOutput = builder.output.require("artifact.txt");
}
"#,
    );

    let diagnostics = run_build(&project, "duplicate", "linux_x86_64", &[])
        .expect_err("a duplicate required-output name must reject");
    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains(
            "required build output `artifact.txt` duplicates or collides with `artifact.txt`"
        ),
        "unexpected diagnostics: {messages}"
    );
}

#[test]
fn invalid_required_output_path_rejects() {
    let project = Project::new("invalid-path");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-output-invalid-path");
    let required: RequiredOutput = builder.output.require("../escape.txt");
}
"#,
    );

    let diagnostics = run_build(&project, "invalid-path", "linux_x86_64", &[])
        .expect_err("a non-canonical required-output path must reject");
    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains("canonical relative components"),
        "unexpected diagnostics: {messages}"
    );
}

#[test]
fn required_output_path_prefix_conflict_rejects() {
    let project = Project::new("prefix-conflict");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-output-prefix-conflict");
    let first: RequiredOutput = builder.output.require("nested");
    let second: RequiredOutput = builder.output.require("nested/artifact.txt");
}
"#,
    );

    let diagnostics = run_build(&project, "prefix-conflict", "linux_x86_64", &[])
        .expect_err("a required output nested inside another must reject");
    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains(
            "required build output `nested/artifact.txt` duplicates or collides with `nested`"
        ),
        "unexpected diagnostics: {messages}"
    );
}

#[test]
fn required_output_mutation_after_completion_rejects() {
    let project = Project::new("sealed-mutation");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-output-sealed-mutation");
    let required: RequiredOutput = builder.output.require("sealed.txt");
    let required_path: &[u8] = required.path();
    let artifact: BuildPath = builder.output.resolve(required_path);
    let descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(descriptor, "first\n");
    let closed: i32 = builder.output.close(descriptor);
    let completion: OutputCompletion = builder.output.complete(required, artifact);
    let reopened: i32 = builder.output.create(artifact, 438);
}
"#,
    );

    let diagnostics = run_build(&project, "sealed-mutation", "linux_x86_64", &[])
        .expect_err("mutating a completed required output must reject");
    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains("required output `sealed.txt`")
            && messages.contains("mutated after its completion"),
        "unexpected diagnostics: {messages}"
    );
}

#[test]
fn fabricated_required_output_carries_no_obligation_authority() {
    let project = Project::new("fabricated");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-output-fabricated");
    let artifact: BuildPath = builder.output.resolve("artifact.txt");
    let descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(descriptor, "x\n");
    let closed: i32 = builder.output.close(descriptor);
    let completion: OutputCompletion = builder.output.complete(RequiredOutput {}, artifact);
}
"#,
    );

    let diagnostics = run_build(&project, "fabricated", "linux_x86_64", &[])
        .expect_err("an authored `RequiredOutput {}` carries no obligation authority");
    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains("no valid obligation index")
            || messages.contains("compiler-issued required output obligation"),
        "unexpected diagnostics: {messages}"
    );
}

#[test]
fn completed_obligation_cannot_complete_again() {
    let project = Project::new("double-complete");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-output-double-complete");
    let required: RequiredOutput = builder.output.require("artifact.txt");
    let required_path: &[u8] = required.path();
    let artifact: BuildPath = builder.output.resolve(required_path);
    let descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(descriptor, "x\n");
    let closed: i32 = builder.output.close(descriptor);
    let first: OutputCompletion = builder.output.complete(required, artifact);
    let second: OutputCompletion = builder.output.complete(required, artifact);
}
"#,
    );

    let diagnostics = run_build(&project, "double-complete", "linux_x86_64", &[])
        .expect_err("completing one obligation twice must reject");
    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains("already completed")
            || messages.contains("required")
            || messages.contains("move")
            || messages.contains("consumed")
            || messages.contains("linear"),
        "unexpected diagnostics: {messages}"
    );
}

#[test]
fn artifact_only_build_publishes_a_completed_required_output() {
    let project = Project::new("artifact-only");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-artifact-only");
    builder.artifact_only();
    let required: RequiredOutput = builder.output.require("report.txt");
    let required_path: &[u8] = required.path();
    let artifact: BuildPath = builder.output.resolve(required_path);
    let descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(descriptor, "report\n");
    let closed: i32 = builder.output.close(descriptor);
    let completion: OutputCompletion = builder.output.complete(required, artifact);
}
"#,
    );

    let checked =
        run_build(&project, "artifact-only", "linux_x86_64", &[]).unwrap_or_else(|diagnostics| {
            panic!(
                "an artifact-only build with a completed required output must publish: {}",
                diagnostic_messages(&diagnostics)
            )
        });
    let observation = checked
        .build_observation_summary()
        .expect("artifact-only execution retains a build observation");
    let settlements = observation.required_output_settlements();
    assert_eq!(settlements.len(), 1, "the artifact obligation settles");
    assert_eq!(settlements[0].relative_path(), b"report.txt");
}

#[test]
fn artifact_only_build_without_a_completed_output_rejects() {
    let project = Project::new("artifact-only-empty");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-artifact-only-empty");
    builder.artifact_only();
}
"#,
    );

    let diagnostics = run_build(&project, "artifact-only-empty", "linux_x86_64", &[])
        .expect_err("an artifact-only build without a completed artifact must reject");
    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains("completed no required output"),
        "unexpected diagnostics: {messages}"
    );
}

#[test]
fn artifact_only_build_rejects_executable_root_bindings() {
    let project = Project::new("artifact-only-roots");
    project.write(
        "main.omg",
        "data Main { }\nmachine Main::main(&mut self) { }\n",
    );
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-artifact-only-roots");
    builder.artifact_only();
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
}
"#,
    );

    let diagnostics = run_build(&project, "artifact-only-roots", "macos_arm64", &[])
        .expect_err("an artifact-only build binding an executable root must reject");
    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains("may not bind executable roots or select boundary providers"),
        "unexpected diagnostics: {messages}"
    );
}

#[test]
fn snapshot_source_lookups_are_narrowed_to_captured_membership() {
    let project = Project::new("negative-lookup");
    project.write("main.omg", "data Main { value: u8; }\n");
    let templates = project.root.join("templates");
    std::fs::create_dir(&templates).expect("create template directory");
    project.write("templates/banner.tmpl", "HELLO {name}\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-negative-lookup");
    let required: RequiredOutput = builder.output.require("lookup.txt");
    let required_path: &[u8] = required.path();
    let artifact: BuildPath = builder.output.resolve(required_path);
    let descriptor: i32 = builder.output.create(artifact, 438);
    let absent: BuildPath = builder.source.resolve("templates/absent.tmpl");
    let absent_descriptor: i32 = builder.source.open(absent, 0);
    transition absent_descriptor < 0 {
        true -> denied(builder, required, artifact, descriptor)
        _ -> opened(builder, required, artifact, descriptor)
    }

    state denied(builder: &mut Build, required: RequiredOutput, artifact: BuildPath, descriptor: i32) {
        let written: i64 = builder.output.write(descriptor, "absent\n");
        let closed: i32 = builder.output.close(descriptor);
        let completion: OutputCompletion = builder.output.complete(required, artifact);
    }

    state opened(builder: &mut Build, required: RequiredOutput, artifact: BuildPath, descriptor: i32) {
        let written: i64 = builder.output.write(descriptor, "present\n");
        let closed: i32 = builder.output.close(descriptor);
        let completion: OutputCompletion = builder.output.complete(required, artifact);
    }
}
"#,
    );

    let checked = run_build(&project, "negative-lookup", "linux_x86_64", &[]).unwrap_or_else(
        |diagnostics| {
            panic!(
                "a snapshot build observing a denied negative lookup must publish: {}",
                diagnostic_messages(&diagnostics)
            )
        },
    );
    let observation = checked
        .build_observation_summary()
        .expect("negative-lookup execution retains a build observation");
    let staged = observation
        .staged_output_tree()
        .expect("completed outputs remain in staged custody");
    let entry = staged
        .sealed_entry(b"lookup.txt")
        .expect("the lookup verdict is discoverable in sealed custody");
    let build_output::BuildStagedOutputEntryKind::File { bytes, .. } = entry.kind() else {
        panic!("the lookup verdict completes only as a sealed regular file")
    };
    assert_eq!(
        bytes, b"absent\n",
        "a member absent from the captured inventory denies the lookup rather than consulting the host"
    );
}

#[test]
fn snapshot_rejects_a_substituted_source_inventory_after_binding() {
    let project = Project::new("substitution");
    project.write("main.omg", "data Main { value: u8; }\n");
    let templates = project.root.join("templates");
    std::fs::create_dir(&templates).expect("create template directory");
    project.write("templates/banner.tmpl", "HELLO {name}\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-substitution");
    let template: BuildPath = builder.source.resolve("templates/banner.tmpl");
    let descriptor: i32 = builder.source.open(template, 0);
    let mut banner_bytes: [u8; 4];
    let read_count: i64 = builder.source.read(descriptor, &mut banner_bytes, 4);
    let closed: i32 = builder.source.close(descriptor);
}
"#,
    );

    let (session, sponsor, build_dir) = bound_build_output_session("substitution");
    set_canonical_source_tree_permissions(&project.root, true);
    // The package binding captures its canonical index over the sealed tree.
    // Substituting a member payload afterward must reject the request before
    // the occurrence is admitted; the snapshot never serves bytes a stale
    // binding did not validate.
    let inputs = package_inputs(&project.root);
    set_canonical_source_tree_permissions(&project.root, false);
    project.write("templates/banner.tmpl", "SUBSTITUTED\n");
    set_canonical_source_tree_permissions(&project.root, true);
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(build_dir),
        package_inputs: Some(inputs),
        filesystem_sponsor: Some(sponsor),
        build_snapshot: Some(build_evaluation::BuildSnapshotRequest::new(
            Vec::<Vec<u8>>::new(),
        )),
        ..CheckedCompileRequest::new(&project.main(), Some("linux_x86_64"))
    })
    .expect_err("a snapshot occurrence must not admit a substituted source inventory");
    set_canonical_source_tree_permissions(&project.root, false);
    let _ = std::fs::remove_dir_all(session);

    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains("no longer matches the complete physical root"),
        "unexpected diagnostics: {messages}"
    );
}

#[test]
fn executable_route_accepts_a_companion_required_output() {
    let project = Project::new("companion");
    project.write(
        "main.omg",
        "data Main { }\nmachine Main::main(&mut self) { }\n",
    );
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-companion");
    let required: RequiredOutput = builder.output.require("companion.txt");
    let required_path: &[u8] = required.path();
    let artifact: BuildPath = builder.output.resolve(required_path);
    let descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(descriptor, "companion\n");
    let closed: i32 = builder.output.close(descriptor);
    let completion: OutputCompletion = builder.output.complete(required, artifact);
}
"#,
    );

    let checked =
        run_build(&project, "companion", "linux_x86_64", &[]).unwrap_or_else(|diagnostics| {
            panic!(
                "an executable build with a completed companion output must publish: {}",
                diagnostic_messages(&diagnostics)
            )
        });
    assert!(
        checked.selected_build_machine_symbol().is_some(),
        "the executable route still retains its selected build machine"
    );
    let observation = checked
        .build_observation_summary()
        .expect("companion execution retains a build observation");
    let settlements = observation.required_output_settlements();
    assert_eq!(settlements.len(), 1, "the companion obligation settles");
    assert_eq!(settlements[0].relative_path(), b"companion.txt");
}
