// Focused coverage for compiler-owned required-output obligations issued
// through `builder.output.require`/`complete`/`fail`, the `artifact_only`
// application modifier, and the executable-with-companion route. Each fixture
// runs the admitted build machine against a fresh sponsored output root bound
// to a captured source snapshot. Later cases pin the snapshot's deterministic
// read surface: canonical roster ordering, metadata-kind enforcement, inert
// symlink members (no traversal, no escape), sticky failure and receipt
// custody, interruption without a committed set, and staged-output capture's
// rejection of entries outside sponsor custody.

use build_declarations::DependencyPurpose;
use checked_interpreter::FilesystemSponsor;
use compiler::CheckedCompileRequest;
use compiler::compile_to_checked;
use package_compilation::{
    BuildDependencyOccurrence, BuildSourceCaptureObligation, BuildSourceCaptureRequest,
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
    capture_scoped_source_input,
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
    // A captured symlink member is inert evidence: its mode lives on the
    // link, and `set_permissions` follows the link to its target, so the
    // walk must skip links entirely rather than chmod a target it does not
    // own (an in-tree directory target would lose its canonical 0o555).
    if metadata.file_type().is_symlink() {
        return;
    }
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
    run_request(
        project,
        label,
        target_name,
        build_evaluation::BuildSnapshotRequest::new(
            required_outputs.iter().map(|name| name.to_vec()),
        ),
        true,
    )
}

/// The package-custody route with a caller-constructed snapshot request.
fn run_build_request(
    project: &Project,
    label: &str,
    target_name: &'static str,
    request: build_evaluation::BuildSnapshotRequest,
) -> Result<compiler::CheckedCompilation, Vec<diagnostics::Diagnostic>> {
    run_request(project, label, target_name, request, true)
}

/// The standalone route: no package custody, so the request must carry its
/// own caller-authorized source inventory.
fn run_standalone_build(
    project: &Project,
    label: &str,
    target_name: &'static str,
    request: build_evaluation::BuildSnapshotRequest,
) -> Result<compiler::CheckedCompilation, Vec<diagnostics::Diagnostic>> {
    run_request(project, label, target_name, request, false)
}

fn run_request(
    project: &Project,
    label: &str,
    target_name: &'static str,
    request: build_evaluation::BuildSnapshotRequest,
    package_custody: bool,
) -> Result<compiler::CheckedCompilation, Vec<diagnostics::Diagnostic>> {
    let (session, sponsor, build_dir) = bound_build_output_session(label);
    set_canonical_source_tree_permissions(&project.root, true);
    // Package custody captures canonical source metadata, so the binding is
    // built only after the fixture is sealed.
    let result = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(build_dir),
        package_inputs: package_custody.then(|| package_inputs(&project.root)),
        filesystem_sponsor: Some(sponsor),
        build_snapshot: Some(request),
        ..CheckedCompileRequest::new(&project.main(), Some(target_name))
    });
    set_canonical_source_tree_permissions(&project.root, false);
    let _ = std::fs::remove_dir_all(session);
    result
}

fn scoped_capture_request(
    entries: &[(&[u8], BuildSourceCaptureObligation)],
) -> BuildSourceCaptureRequest {
    BuildSourceCaptureRequest::new(
        entries
            .iter()
            .map(|(path, obligation)| (path.to_vec(), *obligation)),
    )
    .expect("declared scoped source inventory")
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

    let checked =
        run_build(&project, "negative-lookup", "linux_x86_64", &[]).unwrap_or_else(|diagnostics| {
            panic!(
                "a snapshot build observing a denied negative lookup must publish: {}",
                diagnostic_messages(&diagnostics)
            )
        });
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
fn snapshot_reads_are_deterministic_across_reopens() {
    let project = Project::new("deterministic-reads");
    project.write("main.omg", "data Main { value: u8; }\n");
    let templates = project.root.join("templates");
    std::fs::create_dir(&templates).expect("create template directory");
    project.write("templates/banner.tmpl", "HELLO {name}\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-deterministic-reads");
    let required: RequiredOutput = builder.output.require("verdict.txt");
    let required_path: &[u8] = required.path();
    let artifact: BuildPath = builder.output.resolve(required_path);
    let out: i32 = builder.output.create(artifact, 438);
    let template: BuildPath = builder.source.resolve("templates/banner.tmpl");
    let first: i32 = builder.source.open(template, 0);
    let mut first_bytes: [u8; 4];
    let first_count: i64 = builder.source.read(first, &mut first_bytes, 4);
    let first_closed: i32 = builder.source.close(first);
    let second: i32 = builder.source.open(template, 0);
    let mut second_bytes: [u8; 4];
    let second_count: i64 = builder.source.read(second, &mut second_bytes, 4);
    let second_closed: i32 = builder.source.close(second);
    transition first_count == 4 && second_count == 4 {
        true -> both(builder, required, artifact, out, first_bytes, second_bytes)
        _ -> short(builder, required, artifact, out)
    }

    state both(
        builder: &mut Build,
        required: RequiredOutput,
        artifact: BuildPath,
        out: i32,
        first_bytes: [u8; 4],
        second_bytes: [u8; 4]
    ) {
        let written_first: i64 = builder.output.write(out, &first_bytes);
        let written_second: i64 = builder.output.write(out, &second_bytes);
        let closed: i32 = builder.output.close(out);
        let completion: OutputCompletion = builder.output.complete(required, artifact);
    }

    state short(builder: &mut Build, required: RequiredOutput, artifact: BuildPath, out: i32) {
        let written: i64 = builder.output.write(out, "short\n");
        let closed: i32 = builder.output.close(out);
        let completion: OutputCompletion = builder.output.complete(required, artifact);
    }
}
"#,
    );

    let checked = run_build(&project, "deterministic-reads", "linux_x86_64", &[]).unwrap_or_else(
        |diagnostics| {
            panic!(
                "reopened snapshot reads must publish the build: {}",
                diagnostic_messages(&diagnostics)
            )
        },
    );
    let observation = checked
        .build_observation_summary()
        .expect("deterministic-read execution retains a build observation");
    let staged = observation
        .staged_output_tree()
        .expect("completed outputs remain in staged custody");
    let entry = staged
        .sealed_entry(b"verdict.txt")
        .expect("the verdict output is discoverable in sealed custody");
    let build_output::BuildStagedOutputEntryKind::File { bytes, .. } = entry.kind() else {
        panic!("the verdict output completes only as a sealed regular file")
    };
    // Every fresh descriptor reads the same captured bytes from offset zero:
    // no per-occurrence cursor, host read order, or post-capture drift can
    // make the second open observe different content.
    assert_eq!(
        bytes, b"HELLHELL",
        "both descriptors must serve the identical captured member bytes"
    );
}

#[test]
fn snapshot_directory_member_never_serves_file_bytes() {
    let project = Project::new("directory-read");
    project.write("main.omg", "data Main { value: u8; }\n");
    let templates = project.root.join("templates");
    std::fs::create_dir(&templates).expect("create template directory");
    project.write("templates/banner.tmpl", "HELLO {name}\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-directory-read");
    let required: RequiredOutput = builder.output.require("verdict.txt");
    let required_path: &[u8] = required.path();
    let artifact: BuildPath = builder.output.resolve(required_path);
    let out: i32 = builder.output.create(artifact, 438);
    let directory: BuildPath = builder.source.resolve("templates");
    let descriptor: i32 = builder.source.open(directory, 0);
    transition descriptor < 0 {
        true -> denied(builder, required, artifact, out)
        _ -> reading(builder, required, artifact, out, descriptor)
    }

    state reading(
        builder: &mut Build,
        required: RequiredOutput,
        artifact: BuildPath,
        out: i32,
        descriptor: i32
    ) {
        let mut bytes: [u8; 4];
        let count: i64 = builder.source.read(descriptor, &mut bytes, 4);
        let closed: i32 = builder.source.close(descriptor);
        transition count <= 0 {
            true -> denied(builder, required, artifact, out)
            _ -> served(builder, required, artifact, out)
        }
    }

    state denied(builder: &mut Build, required: RequiredOutput, artifact: BuildPath, out: i32) {
        let written: i64 = builder.output.write(out, "denied\n");
        let closed: i32 = builder.output.close(out);
        let completion: OutputCompletion = builder.output.complete(required, artifact);
    }

    state served(builder: &mut Build, required: RequiredOutput, artifact: BuildPath, out: i32) {
        let written: i64 = builder.output.write(out, "served\n");
        let closed: i32 = builder.output.close(out);
        let completion: OutputCompletion = builder.output.complete(required, artifact);
    }
}
"#,
    );

    let checked =
        run_build(&project, "directory-read", "linux_x86_64", &[]).unwrap_or_else(|diagnostics| {
            panic!(
                "a directory member read probe must still publish the build: {}",
                diagnostic_messages(&diagnostics)
            )
        });
    let observation = checked
        .build_observation_summary()
        .expect("directory-read execution retains a build observation");
    let staged = observation
        .staged_output_tree()
        .expect("completed outputs remain in staged custody");
    let entry = staged
        .sealed_entry(b"verdict.txt")
        .expect("the verdict output is discoverable in sealed custody");
    let build_output::BuildStagedOutputEntryKind::File { bytes, .. } = entry.kind() else {
        panic!("the verdict output completes only as a sealed regular file")
    };
    // The captured inventory records `templates` as a directory row; the
    // canonical kind bound into the Source grant can never serve file bytes
    // through it, whether the host refuses the open or the read.
    assert_eq!(
        bytes, b"denied\n",
        "a captured directory member must never serve file bytes"
    );
}

#[cfg(unix)]
#[test]
fn snapshot_symlink_members_stay_inert_and_deny_escape() {
    use std::os::unix::fs::symlink;

    let project = Project::new("symlink-inert");
    project.write("main.omg", "data Main { value: u8; }\n");
    let templates = project.root.join("templates");
    std::fs::create_dir(&templates).expect("create template directory");
    project.write("templates/banner.tmpl", "HELLO {name}\n");
    // An outside member the escaping link would reach if any traversal
    // followed it: captured spellings are inert evidence, so none may.
    let outside = std::env::temp_dir().join(format!(
        "omega-snapshot-outside-symlink-inert-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&outside);
    std::fs::create_dir(&outside).expect("create outside directory");
    std::fs::write(outside.join("secret.txt"), "SECRET\n").expect("write outside file");
    let escape_target = format!(
        "../{}/secret.txt",
        outside
            .file_name()
            .expect("outside directory name")
            .to_str()
            .expect("outside directory name is UTF-8")
    );
    symlink("templates/banner.tmpl", project.root.join("inside.link"))
        .expect("create in-tree symlink member");
    symlink(&escape_target, project.root.join("escape.link"))
        .expect("create escaping symlink member");
    symlink("templates", project.root.join("linkdir")).expect("create directory symlink member");
    let build_source = r#"machine build(builder: &mut Build) {
    builder.application("snapshot-symlink-inert");
    let required: RequiredOutput = builder.output.require("verdict.txt");
    let required_path: &[u8] = required.path();
    let artifact: BuildPath = builder.output.resolve(required_path);
    let out: i32 = builder.output.create(artifact, 438);
    let inside_path: BuildPath = builder.source.resolve("inside.link");
    let inside: i32 = builder.source.open(inside_path, 0);
    transition inside < 0 {
        true -> denied_inside(builder, required, artifact, out)
        _ -> served(builder, required, artifact, out)
    }

    state denied_inside(builder: &mut Build, required: RequiredOutput, artifact: BuildPath, out: i32) {
        let mark: i64 = builder.output.write(out, "d");
        let escape_path: BuildPath = builder.source.resolve("escape.link");
        let escape: i32 = builder.source.open(escape_path, 0);
        transition escape < 0 {
            true -> denied_escape(builder, required, artifact, out)
            _ -> served(builder, required, artifact, out)
        }
    }

    state denied_escape(builder: &mut Build, required: RequiredOutput, artifact: BuildPath, out: i32) {
        let mark: i64 = builder.output.write(out, "d");
        let traversal_path: BuildPath = builder.source.resolve("linkdir/banner.tmpl");
        let traversal: i32 = builder.source.open(traversal_path, 0);
        transition traversal < 0 {
            true -> denied_traversal(builder, required, artifact, out)
            _ -> served(builder, required, artifact, out)
        }
    }

    state denied_traversal(
        builder: &mut Build,
        required: RequiredOutput,
        artifact: BuildPath,
        out: i32
    ) {
        let mark: i64 = builder.output.write(out, "d");
        let control_path: BuildPath = builder.source.resolve("templates/banner.tmpl");
        let control: i32 = builder.source.open(control_path, 0);
        let mut control_bytes: [u8; 4];
        let control_count: i64 = builder.source.read(control, &mut control_bytes, 4);
        let control_closed: i32 = builder.source.close(control);
        transition control_count == 4 {
            true -> finish(builder, required, artifact, out, control_bytes)
            _ -> served(builder, required, artifact, out)
        }
    }

    state finish(
        builder: &mut Build,
        required: RequiredOutput,
        artifact: BuildPath,
        out: i32,
        control_bytes: [u8; 4]
    ) {
        let written: i64 = builder.output.write(out, &control_bytes);
        let closed: i32 = builder.output.close(out);
        let completion: OutputCompletion = builder.output.complete(required, artifact);
    }

    state served(builder: &mut Build, required: RequiredOutput, artifact: BuildPath, out: i32) {
        let written: i64 = builder.output.write(out, "SERVED\n");
        let closed: i32 = builder.output.close(out);
        let completion: OutputCompletion = builder.output.complete(required, artifact);
    }
}
"#;
    project.write("build.omg", build_source);

    let checked =
        run_build(&project, "symlink-inert", "linux_x86_64", &[]).unwrap_or_else(|diagnostics| {
            panic!(
                "inert-link traversal probes must still publish the build: {}",
                diagnostic_messages(&diagnostics)
            )
        });
    let _ = std::fs::remove_dir_all(&outside);
    let observation = checked
        .build_observation_summary()
        .expect("inert-link execution retains a build observation");
    // The three link members are real captured inventory rows; their extent
    // evidence is counted even though no host link is ever materialized.
    let inventory = observation
        .captured_source_inventory()
        .expect("captured inventory extent evidence is recorded");
    assert_eq!(
        inventory.entry_count(),
        8,
        "root + main.omg + build.omg + templates + banner.tmpl + three link members"
    );
    assert_eq!(
        inventory.file_bytes(),
        "data Main { value: u8; }\n".len() as u64
            + build_source.len() as u64
            + "HELLO {name}\n".len() as u64,
        "retained file bytes exclude inert link spellings"
    );
    let staged = observation
        .staged_output_tree()
        .expect("completed outputs remain in staged custody");
    let entry = staged
        .sealed_entry(b"verdict.txt")
        .expect("the verdict output is discoverable in sealed custody");
    let build_output::BuildStagedOutputEntryKind::File { bytes, .. } = entry.kind() else {
        panic!("the verdict output completes only as a sealed regular file")
    };
    // `d` per denied probe, then the control member's own captured bytes: an
    // in-tree link, an escaping link, and a traversal through a link member
    // all deny, while the ordinary member still serves its captured bytes.
    assert_eq!(
        bytes, b"dddHELL",
        "captured link members must stay inert: no traversal, no escape, no substitution"
    );
}

#[test]
fn omitted_required_outputs_report_in_canonical_order() {
    let project = Project::new("roster-order");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-roster-order");
    let artifact: BuildPath = builder.output.resolve("z-last.txt");
    let descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(descriptor, "z\n");
    let closed: i32 = builder.output.close(descriptor);
}
"#,
    );

    // The declared roster is deliberately unsorted: the linear check must
    // report the canonically first omitted member, not the first declared
    // missing one, so ordering evidence never depends on request order.
    let diagnostics = run_build(
        &project,
        "roster-order",
        "linux_x86_64",
        &[&b"m-mid.txt"[..], &b"z-last.txt"[..], &b"a-first.txt"[..]],
    )
    .expect_err("a roster with omitted members must reject publication");
    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains("required build output `a-first.txt`")
            && messages.contains("omitted from the sealed staged-output tree"),
        "unexpected diagnostics: {messages}"
    );
    assert!(
        !messages.contains("m-mid.txt"),
        "the canonically first omitted member is named first: {messages}"
    );
}

#[test]
fn failed_obligation_stays_failed_and_cannot_complete() {
    let project = Project::new("sticky-fail");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-sticky-fail");
    let required: RequiredOutput = builder.output.require("artifact.txt");
    builder.output.fail(required, "generator unavailable");
    let artifact: BuildPath = builder.output.resolve("artifact.txt");
    let descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(descriptor, "late\n");
    let closed: i32 = builder.output.close(descriptor);
    let completion: OutputCompletion = builder.output.complete(required, artifact);
}
"#,
    );

    let diagnostics = run_build(&project, "sticky-fail", "linux_x86_64", &[])
        .expect_err("a failed obligation must stay failed; a later completion cannot revive it");
    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains("already failed")
            || messages.contains("required")
            || messages.contains("move")
            || messages.contains("consumed")
            || messages.contains("linear"),
        "unexpected diagnostics: {messages}"
    );
}

#[test]
fn complete_rejects_another_obligations_reserved_output() {
    let project = Project::new("cross-obligation");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-cross-obligation");
    let first: RequiredOutput = builder.output.require("first.txt");
    let second: RequiredOutput = builder.output.require("second.txt");
    let second_path: &[u8] = second.path();
    let second_file: BuildPath = builder.output.resolve(second_path);
    let descriptor: i32 = builder.output.create(second_file, 438);
    let written: i64 = builder.output.write(descriptor, "second\n");
    let closed: i32 = builder.output.close(descriptor);
    let wrong: OutputCompletion = builder.output.complete(first, second_file);
}
"#,
    );

    let diagnostics = run_build(&project, "cross-obligation", "linux_x86_64", &[])
        .expect_err("a sealed file must never be rejoined to a different obligation's receipt");
    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains("reserved output"),
        "unexpected diagnostics: {messages}"
    );
}

#[test]
fn interrupted_activation_commits_no_output_set() {
    let project = Project::new("interruption");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-interruption");
    let required: RequiredOutput = builder.output.require("artifact.txt");
    let required_path: &[u8] = required.path();
    let artifact: BuildPath = builder.output.resolve(required_path);
    let descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(descriptor, "partial\n");
    let closed: i32 = builder.output.close(descriptor);
    let escape: BuildPath = builder.output.resolve("../escape.txt");
}
"#,
    );

    let diagnostics = run_build(&project, "interruption", "linux_x86_64", &[]).expect_err(
        "an activation that halts mid-run must reject rather than commit a partial set",
    );
    let messages = diagnostic_messages(&diagnostics);
    // `artifact.txt` was already sealed into staged custody when the machine
    // halted on the noncanonical resolve; rejection here means no observation
    // or committed output set escaped the interrupted occurrence.
    assert!(
        messages.contains("build-time evaluation of `build` failed")
            && messages.contains("canonical relative components"),
        "unexpected diagnostics: {messages}"
    );
}

#[test]
fn staged_output_capture_rejects_foreign_uncommitted_entries() {
    let project = Project::new("foreign-entry");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-foreign-entry");
    let required: RequiredOutput = builder.output.require("artifact.txt");
    let required_path: &[u8] = required.path();
    let artifact: BuildPath = builder.output.resolve(required_path);
    let descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(descriptor, "artifact\n");
    let closed: i32 = builder.output.close(descriptor);
    let completion: OutputCompletion = builder.output.complete(required, artifact);
}
"#,
    );

    let (session, sponsor, build_dir) = bound_build_output_session("foreign-entry");
    // A member another occurrence could have left in the output root: staged
    // custody is exactly this occurrence's sponsor-committed entries, so a
    // foreign residue poisons the capture rather than silently joining the
    // committed set.
    std::fs::write(build_dir.join("foreign.txt"), "foreign\n")
        .expect("plant a foreign staged entry");
    set_canonical_source_tree_permissions(&project.root, true);
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(build_dir),
        package_inputs: Some(package_inputs(&project.root)),
        filesystem_sponsor: Some(sponsor),
        build_snapshot: Some(build_evaluation::BuildSnapshotRequest::new(
            Vec::<Vec<u8>>::new(),
        )),
        ..CheckedCompileRequest::new(&project.main(), Some("linux_x86_64"))
    })
    .expect_err("a staged entry outside sponsor custody must reject the committed set");
    set_canonical_source_tree_permissions(&project.root, false);
    let _ = std::fs::remove_dir_all(session);

    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains("absent from sponsor custody"),
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

/// The exact application root, declaration role, and requested target a
/// retained native product publishes under. `linux_x86_64` and
/// `windows_x86_64` both bind so a same-fixture foreign-target realization can
/// stand in for target/artifact drift below.
fn write_native_generated_project(project: &Project) {
    project.write(
        "main.omg",
        "data Main { }\nmachine Main::main(&mut self) { }\n",
    );
    project.write("input.txt", "input\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-native-product");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    let input: BuildPath = builder.source.resolve("input.txt");
    let input_descriptor: i32 = builder.source.open(input, 0);
    let mut input_bytes: [u8; 6];
    let input_count: i64 = builder.source.read(input_descriptor, &mut input_bytes, 6);
    let input_close: i32 = builder.source.close(input_descriptor);

    let generated: BuildPath = builder.output.resolve("generated.omg");
    let descriptor: i32 = builder.output.create(generated, 438);
    let written: i64 = builder.output.write(
        descriptor,
        "pub data ReplayGenerated { value: u64; }\n"
    );
    let closed: i32 = builder.output.close(descriptor);
    builder.output.include_source(generated);

    let required: RequiredOutput = builder.output.require("artifact.txt");
    let required_path: &[u8] = required.path();
    let artifact: BuildPath = builder.output.resolve(required_path);
    let artifact_descriptor: i32 = builder.output.create(artifact, 438);
    let artifact_written: i64 = builder.output.write(artifact_descriptor, "banner\n");
    let artifact_closed: i32 = builder.output.close(artifact_descriptor);
    let completion: OutputCompletion = builder.output.complete(required, artifact);
}
"#,
    );
}

/// The same single-package graph as `package_inputs`, declared with the
/// authored Application role a retained native product must bind.
fn application_inputs(root: &Path) -> PackageCompilationInputs {
    let package = PackageKeyIdentity::from_digest([89; 32]).expect("nonzero package identity");
    PackageCompilationInputs::new(
        package,
        package_compilation::BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(package, "snapshot-outputs", root.to_path_buf())
                .with_canonical_source_metadata()
                .expect("capture canonical package source"),
        ],
        Vec::<PackageDependencyBinding>::new(),
    )
    .expect("single-package application input")
}

fn checked_native_product(
    project: &Project,
    target_name: &'static str,
    label: &str,
) -> compiler::CheckedCompilation {
    let (session, sponsor, build_dir) = bound_build_output_session(label);
    set_canonical_source_tree_permissions(&project.root, true);
    let result = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(build_dir),
        package_inputs: Some(application_inputs(&project.root)),
        filesystem_sponsor: Some(sponsor),
        ..CheckedCompileRequest::new(&project.main(), Some(target_name))
    });
    set_canonical_source_tree_permissions(&project.root, false);
    let _ = std::fs::remove_dir_all(session);
    result.unwrap_or_else(|diagnostics| {
        panic!(
            "the generated-source application compiles for {target_name}: {}",
            diagnostic_messages(&diagnostics)
        )
    })
}

/// One checked compilation's retained native product, realized but not yet
/// assembled into a report: the exact production subject and retained artifact
/// a publishable report must bind, with the proposal's own publication
/// metadata.
struct RealizedNativeProduct {
    root_path: PathBuf,
    source_file_count: usize,
    rollback: Option<compiler::OptimizationRollbackReceipt>,
    subject: Option<compiler::ProductionCompilationSubject>,
    artifact: compiler::RetainedNativeArtifact,
    application_name: Option<String>,
    application_intent: Option<build_evaluation::HostedApplicationIntent>,
    application_identifier: Option<build_evaluation::ApplicationIdentifier>,
}

/// Drive the checked product through the same compiler-owned join the package
/// manager uses: retained Terminal report, proposal-checked native
/// realization. Nothing here restages output or reopens source custody.
fn realize_checked_native_product(
    project: &Project,
    checked: compiler::CheckedCompilation,
) -> RealizedNativeProduct {
    let profile = proof_admission::AdmissionProfile::default();
    let report = compiler::retained_terminal_report_from_checked_package(
        project.main(),
        checked,
        profile.clone(),
    )
    .unwrap_or_else(|diagnostics| {
        panic!(
            "the checked product retains its Terminal report: {}",
            diagnostic_messages(&diagnostics)
        )
    });
    let root_path = report.root_path().to_path_buf();
    let source_file_count = report.source_file_count;
    let rollback = report.optimization_rollback_receipt().cloned();
    let subject = report
        .production_manifest()
        .map(|manifest| manifest.subject().clone());
    let retained = report
        .into_retained_terminal_artifact()
        .expect("the package report retains its Terminal product");
    let proposal = retained
        .native_realization_proposal()
        .expect("the retained Terminal product carries a native proposal");
    let subsystem = proposal.subsystem();
    let optimization_selections = proposal.post_terminal_optimizations().selections().clone();
    let application_name = proposal.application_name().map(str::to_owned);
    let application_intent = proposal.application_intent();
    let application_identifier = proposal.application_identifier().cloned();
    let artifact = compiler::realize_retained_native_artifact(
        retained,
        compiler::RetainedNativeRealizationRequest {
            profile: &profile,
            optimization_selections: &optimization_selections,
            terminal_authority_policy: native_realization::current_terminal_authority_policy(),
            accepted_package_terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            terminal_authority_permission_policy: Some(
                native_realization::current_terminal_authority_permission_policy(),
            ),
            image_request: native_realization::ExecutableImageEmissionRequest::direct(subsystem),
            imports: &[],
        },
    )
    .unwrap_or_else(|(_, diagnostics)| {
        panic!(
            "the retained product realizes natively: {}",
            diagnostic_messages(&diagnostics)
        )
    });
    let artifact = artifact
        .into_direct()
        .unwrap_or_else(|_| panic!("the flat image request keeps direct artifact custody"));
    RealizedNativeProduct {
        root_path,
        source_file_count,
        rollback,
        subject,
        artifact,
        application_name,
        application_intent,
        application_identifier,
    }
}

/// Assemble the retained artifact into its production report and publish it.
/// The report binds the subject/artifact join before any bytes install.
fn publish_native_product(
    product: RealizedNativeProduct,
    build_dir: &Path,
) -> (compiler::CompileReport, PathBuf) {
    let report = compiler::CompileReport::from_retained_native_artifact(
        product.root_path,
        product.source_file_count,
        product.artifact,
        product.rollback,
        product.subject,
    )
    .expect("the retained native artifact assembles its production report")
    .with_application_metadata(
        product.application_name,
        product.application_intent,
        product.application_identifier,
    )
    .expect("the retained publication metadata joins the report");
    let published = report
        .publish_retained_native_artifact(build_dir)
        .expect("the validated retained native product publishes");
    let executable = published
        .checked_native_executable_path()
        .expect("publication retains exact executable custody")
        .to_path_buf();
    (published, executable)
}

/// Acceptance: a serialized replay of the admitted activation reproduces the
/// retained native product built with generated source — the identical
/// consumed source commitment, generated-source bundle, production subject,
/// and published executable bytes.
#[test]
fn serialized_replay_reproduces_the_retained_native_product() {
    let project = Project::new("replay-product");
    write_native_generated_project(&project);
    let (session, sponsor, build_dir) = bound_build_output_session("replay-product");
    set_canonical_source_tree_permissions(&project.root, true);
    let inputs = application_inputs(&project.root);
    let checked = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(build_dir),
        package_inputs: Some(inputs.clone()),
        filesystem_sponsor: Some(sponsor),
        ..CheckedCompileRequest::new(&project.main(), Some("linux_x86_64"))
    })
    .unwrap_or_else(|diagnostics| {
        panic!(
            "the generated-source application compiles through admitted build custody: {}",
            diagnostic_messages(&diagnostics)
        )
    });
    assert!(
        checked
            .typed
            .data_definitions()
            .iter()
            .any(|definition| definition.name.as_str() == "ReplayGenerated"),
        "the build's generated source joins the checked product"
    );
    assert!(
        checked
            .package_generated_source_bundle()
            .map(|bundle| !bundle.sources().is_empty())
            .unwrap_or(false),
        "the checked product retains the generated-source bundle"
    );

    // The activation's review-only record is canonical bytes: serialize it,
    // recover it, and replay the complete activation with no staged output or
    // sponsor authority. Replay reproduces the identical consumed inputs.
    let summary = checked
        .build_observation_summary()
        .expect("admitted activation retains observation custody");
    assert!(
        summary.filesystem_replay_verdict().is_complete(),
        "primary activation completes its internal verifier replay"
    );
    let limits = build_evaluation::BuildFilesystemReplayRecordLimits::default();
    let record = build_evaluation::capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("capture the verified replay record")
        .expect("a complete receipted activation issues a replay record");
    let recovered = build_evaluation::recover_review_only_build_filesystem_replay_record(
        record.canonical_bytes(),
        limits,
    )
    .expect("serialized replay record recovers");
    let replayed = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs.clone()),
        replay_record: Some(recovered),
        ..CheckedCompileRequest::new(&project.main(), Some("linux_x86_64"))
    })
    .expect("serialized replay reproduces the admitted activation");
    set_canonical_source_tree_permissions(&project.root, false);
    let _ = std::fs::remove_dir_all(&session);
    assert_eq!(
        replayed.source_consumption_commitment(),
        checked.source_consumption_commitment(),
        "the replayed activation consumes identical authored and generated source"
    );
    assert_eq!(
        replayed
            .package_generated_source_bundle()
            .expect("replayed activation retains its generated-source bundle")
            .sources(),
        checked
            .package_generated_source_bundle()
            .expect("primary activation retains its generated-source bundle")
            .sources(),
        "replay retains the identical generated-source custody"
    );

    // Both checked products carry the identical production subject — root,
    // authored role, target profile, build usage, and observation identity —
    // and publish byte-identical executables.
    let primary = realize_checked_native_product(&project, checked);
    let replayed = realize_checked_native_product(&project, replayed);
    let subject = primary
        .subject
        .as_ref()
        .expect("the application product carries its production subject");
    assert_eq!(
        subject.package().root(),
        PackageKeyIdentity::from_digest([89; 32]).unwrap()
    );
    assert_eq!(
        subject.package().root_role(),
        package_compilation::BuildDeclarationKind::Application
    );
    assert_eq!(subject.target_profile(), target::TargetProfile::LinuxX64);
    assert_eq!(
        replayed.subject.as_ref(),
        Some(subject),
        "replay binds the identical production subject"
    );
    let publish_root = std::env::temp_dir().join(format!(
        "omega-snapshot-outputs-publish-replay-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&publish_root);
    let (_primary_report, primary_executable) =
        publish_native_product(primary, &publish_root.join("primary"));
    let (_replayed_report, replayed_executable) =
        publish_native_product(replayed, &publish_root.join("replayed"));
    assert_eq!(
        std::fs::read(&primary_executable).expect("read the primary executable"),
        std::fs::read(&replayed_executable).expect("read the replayed executable"),
        "serialized replay reproduces the identical retained native product"
    );
    let _ = std::fs::remove_dir_all(&publish_root);
}

/// The retained-product join is also the publication drift boundary. A checked
/// compilation whose authored source moved cannot produce its report; a
/// foreign-target artifact cannot assemble a report under this subject; and a
/// foreign artifact supplies no physical evidence to this manifest. A report
/// that never assembles can never publish.
#[test]
fn retained_native_product_publication_rejects_source_target_and_artifact_drift() {
    let project = Project::new("drift-product");
    write_native_generated_project(&project);

    let checked = checked_native_product(&project, "linux_x86_64", "drift-linux");
    let product = realize_checked_native_product(&project, checked);
    let linux_subject = product
        .subject
        .clone()
        .expect("the application product carries its production subject");
    let publish_root = std::env::temp_dir().join(format!(
        "omega-snapshot-outputs-publish-drift-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&publish_root);
    let (published, executable) = publish_native_product(product, &publish_root.join("linux"));
    assert!(executable.is_file(), "the retained product installed bytes");
    let manifest = published
        .production_manifest()
        .expect("the published report retains its production manifest");

    // Target and artifact drift: the same project's windows realization can
    // neither join the linux production subject nor supply its physical
    // evidence.
    let windows = realize_checked_native_product(
        &project,
        checked_native_product(&project, "windows_x86_64", "drift-windows"),
    );
    assert!(
        !manifest.matches_native_artifact(&windows.artifact),
        "a foreign-target artifact must not match this manifest"
    );
    assert!(
        matches!(
            manifest.require_native_physical_evidence(&windows.artifact),
            Err(compiler::FinalRealizationEvidenceError::NativeTargetMismatch)
        ),
        "a foreign-target artifact supplies no physical evidence"
    );
    let message = compiler::CompileReport::from_retained_native_artifact(
        windows.root_path,
        windows.source_file_count,
        windows.artifact,
        windows.rollback,
        Some(linux_subject),
    )
    .expect_err("a foreign-target artifact cannot assemble this subject's report");
    assert!(
        message.contains("target"),
        "unexpected report assembly error: {message}"
    );

    // Source drift: a checked compilation whose authored source moved before
    // its retained product was cut cannot produce the report publication
    // needs.
    let drifted = checked_native_product(&project, "linux_x86_64", "drift-source");
    project.write(
        "main.omg",
        "data Main { }\nmachine Main::main(&mut self) { let drifted: u64 = 0; }\n",
    );
    let diagnostics = compiler::retained_terminal_report_from_checked_package(
        project.main(),
        drifted,
        proof_admission::AdmissionProfile::default(),
    )
    .expect_err("drifted authored source must reject before the retained product");
    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains("changed after frontend loading"),
        "unexpected diagnostics: {messages}"
    );
    let _ = std::fs::remove_dir_all(&publish_root);
}

#[test]
fn standalone_scoped_capture_serves_only_declared_members() {
    let project = Project::new("scoped-members");
    project.write("main.omg", "data Main { value: u8; }\n");
    let templates = project.root.join("templates");
    std::fs::create_dir(&templates).expect("create template directory");
    project.write("templates/banner.tmpl", "HELLO {name}\n");
    // A real sibling the working directory holds but the caller never
    // authorized: the scoped inventory must deny it even though capture on
    // this host could physically read it.
    project.write("ledger.txt", "LEDGER\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-scoped-members");
    let required: RequiredOutput = builder.output.require("verdict.txt");
    let required_path: &[u8] = required.path();
    let artifact: BuildPath = builder.output.resolve(required_path);
    let out: i32 = builder.output.create(artifact, 438);
    let member_path: BuildPath = builder.source.resolve("templates/banner.tmpl");
    let member: i32 = builder.source.open(member_path, 0);
    transition member < 0 {
        true -> failed(builder, required, artifact, out)
        _ -> reading(builder, required, artifact, out, member)
    }

    state reading(
        builder: &mut Build,
        required: RequiredOutput,
        artifact: BuildPath,
        out: i32,
        member: i32
    ) {
        let mut bytes: [u8; 4];
        let count: i64 = builder.source.read(member, &mut bytes, 4);
        let closed: i32 = builder.source.close(member);
        transition count == 4 {
            true -> probing(builder, required, artifact, out, bytes)
            _ -> failed(builder, required, artifact, out)
        }
    }

    state probing(
        builder: &mut Build,
        required: RequiredOutput,
        artifact: BuildPath,
        out: i32,
        bytes: [u8; 4]
    ) {
        let undeclared_path: BuildPath = builder.source.resolve("ledger.txt");
        let undeclared: i32 = builder.source.open(undeclared_path, 0);
        transition undeclared < 0 {
            true -> finish(builder, required, artifact, out, bytes)
            _ -> failed(builder, required, artifact, out)
        }
    }

    state finish(
        builder: &mut Build,
        required: RequiredOutput,
        artifact: BuildPath,
        out: i32,
        bytes: [u8; 4]
    ) {
        let written: i64 = builder.output.write(out, &bytes);
        let mark: i64 = builder.output.write(out, "d\n");
        let closed: i32 = builder.output.close(out);
        let completion: OutputCompletion = builder.output.complete(required, artifact);
    }

    state failed(builder: &mut Build, required: RequiredOutput, artifact: BuildPath, out: i32) {
        let written: i64 = builder.output.write(out, "FAIL\n");
        let closed: i32 = builder.output.close(out);
        let completion: OutputCompletion = builder.output.complete(required, artifact);
    }
}
"#,
    );

    // The standalone root names its whole inventory explicitly: the two
    // already-required source files and the one declared template subtree.
    // `ledger.txt` stays physically present but outside the inventory.
    let checked = run_standalone_build(
        &project,
        "scoped-members",
        "linux_x86_64",
        build_evaluation::BuildSnapshotRequest::scoped(
            std::iter::empty::<Vec<u8>>(),
            scoped_capture_request(&[
                (b"build.omg", BuildSourceCaptureObligation::Required),
                (b"main.omg", BuildSourceCaptureObligation::Required),
                (b"templates", BuildSourceCaptureObligation::Required),
            ]),
        ),
    )
    .unwrap_or_else(|diagnostics| {
        panic!(
            "a scoped standalone build must publish against its declared inventory: {}",
            diagnostic_messages(&diagnostics)
        )
    });
    let observation = checked
        .build_observation_summary()
        .expect("scoped standalone execution retains a build observation");
    let staged = observation
        .staged_output_tree()
        .expect("completed outputs remain in staged custody");
    let entry = staged
        .sealed_entry(b"verdict.txt")
        .expect("the verdict output is discoverable in sealed custody");
    let build_output::BuildStagedOutputEntryKind::File { bytes, .. } = entry.kind() else {
        panic!("the verdict output completes only as a sealed regular file")
    };
    // The declared subtree member serves its captured bytes; the undeclared
    // sibling — physically present in the working directory — is denied.
    assert_eq!(
        bytes, b"HELLd\n",
        "a scoped snapshot admits declared members and denies every undeclared sibling"
    );
}

#[test]
fn standalone_scoped_capture_rejects_a_missing_required_member() {
    let project = Project::new("scoped-absent");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-scoped-absent");
}
"#,
    );

    // `ghost.txt` is declared Required but never written: capture must fail
    // rather than silently narrow the inventory.
    let diagnostics = run_standalone_build(
        &project,
        "scoped-absent",
        "linux_x86_64",
        build_evaluation::BuildSnapshotRequest::scoped(
            std::iter::empty::<Vec<u8>>(),
            scoped_capture_request(&[
                (b"build.omg", BuildSourceCaptureObligation::Required),
                (b"ghost.txt", BuildSourceCaptureObligation::Required),
                (b"main.omg", BuildSourceCaptureObligation::Required),
            ]),
        ),
    )
    .expect_err("a missing required capture entry must reject");
    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains("required source capture entry `ghost.txt` is absent"),
        "unexpected diagnostics: {messages}"
    );
}

#[test]
fn standalone_scoped_capture_rejects_a_consumed_member_the_request_omits() {
    let project = Project::new("scoped-omits");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-scoped-omits");
}
"#,
    );

    // The request declares only `main.omg`; `build.omg` is a source member
    // this compilation already consumed, so the inventory cannot omit it.
    let diagnostics = run_standalone_build(
        &project,
        "scoped-omits",
        "linux_x86_64",
        build_evaluation::BuildSnapshotRequest::scoped(
            std::iter::empty::<Vec<u8>>(),
            scoped_capture_request(&[(b"main.omg", BuildSourceCaptureObligation::Required)]),
        ),
    )
    .expect_err("a scoped inventory that omits a required source member must reject");
    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains("scoped source inventory omits required source member"),
        "unexpected diagnostics: {messages}"
    );
}

#[test]
fn named_input_for_an_absent_dependency_occurrence_rejects() {
    let project = Project::new("absent-occurrence");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-absent-occurrence");
}
"#,
    );

    // The input itself is honestly captured, but it is keyed to an edge the
    // reconciled graph does not contain — an extra input the binding must
    // reject rather than attach to another occurrence.
    set_canonical_source_tree_permissions(&project.root, true);
    let input = capture_scoped_source_input(
        &project.root,
        &scoped_capture_request(&[(b"main.omg", BuildSourceCaptureObligation::Required)]),
        Vec::<PathBuf>::new(),
    )
    .expect("capture the named input's own inventory");
    set_canonical_source_tree_permissions(&project.root, false);
    let root = PackageKeyIdentity::from_digest([89; 32]).expect("nonzero package identity");
    let ghost_target = PackageKeyIdentity::from_digest([90; 32]).expect("nonzero package identity");
    let request = build_evaluation::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>())
        .with_dependency_inputs([(
            BuildDependencyOccurrence::new(root, DependencyPurpose::Build, "ghost", ghost_target),
            b"module".to_vec(),
            input,
        )])
        .expect("the named-input map is well formed");

    let diagnostics = run_build_request(&project, "absent-occurrence", "linux_x86_64", request)
        .expect_err("an input assigned to an absent occurrence must reject");
    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains("dependency occurrence that does not exist")
            && messages.contains("ghost"),
        "unexpected diagnostics: {messages}"
    );
}

#[test]
fn standalone_snapshot_without_a_capture_request_rejects() {
    let project = Project::new("unscoped");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write("notes.txt", "PRIVATE\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("snapshot-unscoped");
}
"#,
    );

    // The package-inventory form names no members; without package custody it
    // would silently capture the whole working directory, so it rejects.
    let diagnostics = run_standalone_build(
        &project,
        "unscoped",
        "linux_x86_64",
        build_evaluation::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>()),
    )
    .expect_err("a standalone snapshot with no capture request must reject");
    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains("requires package source custody"),
        "unexpected diagnostics: {messages}"
    );
}
