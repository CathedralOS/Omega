//! `inspect-terminal` prepares a package project through the same manager
//! route as `--check`, and a standalone root keeps its direct local resolution.

use super::{inspect, inspect_on_target};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// A scratch tree of authored sources, removed on drop. It deliberately owns
/// nothing under the resolver's per-user snapshot cache.
struct ScratchProject(PathBuf);

impl ScratchProject {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "omega-inspect-terminal-{name}-{}-{stamp}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).expect("create inspect-terminal scratch project");
        Self(std::fs::canonicalize(path).expect("canonical scratch project path"))
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().expect("scratch file has a parent"))
            .expect("create scratch project directory");
        std::fs::write(&path, contents).expect("write scratch project source");
        path
    }

    fn read_optional(&self, relative: &str) -> Option<Vec<u8>> {
        match std::fs::read(self.0.join(relative)) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => panic!("read scratch project file {relative}: {error}"),
        }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }

    fn create_dir(&self, relative: &str) -> PathBuf {
        let path = self.path(relative);
        std::fs::create_dir(&path).expect("create scratch project directory");
        path
    }
}

impl Drop for ScratchProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const ROOT_SOURCE: &str = r#"use facts::x;

data Root {}

machine Root::forward(token: Token) {
    transition { _ -> done(token) }
    state done(token: Token) {}
}
"#;

const DEPENDENCY_MODULE: &str = "pub data Token { value: i32; }\n";

#[test]
fn packaged_root_resolves_declared_path_dependency_imports() {
    let project = ScratchProject::new("packaged");
    project.write(
        "root/build.omg",
        r#"machine build(builder: &mut Build) {
    builder.package("inspect-root");
    builder.depend_as("facts", Source::Path { location: "../facts" });
}
"#,
    );
    let entry = project.write("root/main.omg", ROOT_SOURCE);
    project.write(
        "facts/build.omg",
        "machine build(builder: &mut Build) {\n    builder.package(\"facts\");\n}\n",
    );
    project.write("facts/main.omg", "pub machine value() -> u64 { 7 }\n");
    project.write("facts/x.omg", DEPENDENCY_MODULE);
    let declaration = project.read_optional("root/build.omg");

    let output = inspect("Root::forward", &entry);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "packaged inspection must resolve `facts` through the declared dependency: {stderr}"
    );
    assert!(
        !stderr.contains("failed to resolve"),
        "the alias was resolved as a local path: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("selected_machine=Root::forward"),
        "{stdout}"
    );
    assert!(
        stdout.contains("identity=named(name(Token))"),
        "the dependency's type must reach the terminal module: {stdout}"
    );
    // Inspection reads project policy; it never publishes or rewrites it.
    assert_eq!(project.read_optional("root/build.omg"), declaration);
    assert_eq!(project.read_optional("root/omega.lock"), None);
}

#[test]
fn standalone_root_keeps_resolving_sibling_path_imports() {
    let project = ScratchProject::new("standalone");
    let entry = project.write("main.omg", ROOT_SOURCE);
    project.write("facts/x.omg", DEPENDENCY_MODULE);

    let output = inspect("Root::forward", &entry);

    assert!(
        output.status.success(),
        "a root without build.omg must keep the direct local check: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("selected_machine=Root::forward"),
        "{stdout}"
    );
    assert!(stdout.contains("identity=named(name(Token))"), "{stdout}");
    assert_eq!(project.read_optional("omega.lock"), None);
}

/// A packaged build that reaches its Source facet: reads a template through
/// the captured snapshot. Package inspection uses the manager's sponsored
/// candidate session, whose staged outputs and captured inputs remain private.
const SNAPSHOT_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.package("inspect-snapshot-root");
    let template: BuildPath = builder.source.resolve("templates/banner.tmpl");
    let template_descriptor: i32 = builder.source.open(template, 0);
    let mut banner_bytes: [u8; 7];
    let read_count: i64 = builder.source.read(template_descriptor, &mut banner_bytes, 7);
    let source_close: i32 = builder.source.close(template_descriptor);
}
"#;

fn snapshot_project() -> (ScratchProject, PathBuf) {
    let project = ScratchProject::new("snapshot");
    project.write("root/build.omg", SNAPSHOT_BUILD);
    let entry = project.write("root/main.omg", "machine main() {}\n");
    project.write("root/templates/banner.tmpl", "HELLO {{name}}\n");
    (project, entry)
}

/// The bound route materializes the occurrence's captured inventory under the
/// child's temp root before the build machine runs. A temp root that cannot
/// hold the private snapshot fails the inspection; an unbound route would
/// have reached the shared resolver snapshot instead and succeeded.
#[test]
fn packaged_root_build_materializes_a_private_captured_source_snapshot() {
    let (project, entry) = snapshot_project();
    let missing_temp = project.path("scratch/missing");

    let output = inspect_on_target(
        "main",
        &entry,
        "linux_x86_64",
        &[("TMPDIR", missing_temp.as_path())],
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains("failed to create the captured source snapshot"),
        "the bound snapshot must materialize under the child's temp root: {stderr}"
    );
}

/// The same fixture under a usable private temp root completes its build
/// activation: the materialized snapshot is released on exit and nothing
/// about the authored project is rewritten.
#[test]
fn packaged_root_build_releases_its_private_captured_source_snapshot() {
    let (project, entry) = snapshot_project();
    let scratch = project.create_dir("scratch");
    let declaration = project.read_optional("root/build.omg");

    let output = inspect_on_target(
        "main",
        &entry,
        "linux_x86_64",
        &[("TMPDIR", scratch.as_path())],
    );

    assert!(
        output.status.success(),
        "packaged snapshot-bound inspection failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("selected_machine=main"), "{stdout}");
    let residue = std::fs::read_dir(&scratch)
        .expect("read fixture temp root")
        .map(|entry| entry.expect("temp entry").file_name())
        .collect::<Vec<_>>();
    assert_eq!(
        residue,
        Vec::<std::ffi::OsString>::new(),
        "the occurrence must release its private captured-source snapshot"
    );
    assert_eq!(project.read_optional("root/build.omg"), declaration);
}

/// A standalone root has no package inputs and binds no captured snapshot:
/// the same unusable temp root cannot affect it.
#[test]
fn standalone_root_never_materializes_a_captured_source_snapshot() {
    let project = ScratchProject::new("standalone-unbound");
    let entry = project.write("main.omg", "machine main() {}\n");
    let missing_temp = project.path("missing-temp");

    let output = inspect_on_target(
        "main",
        &entry,
        "linux_x86_64",
        &[("TMPDIR", missing_temp.as_path())],
    );

    assert!(
        output.status.success(),
        "a standalone inspection must not depend on the captured-snapshot temp root: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("selected_machine=main"), "{stdout}");
}
