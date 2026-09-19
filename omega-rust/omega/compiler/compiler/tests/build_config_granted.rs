//! Fixtures shared by the build config tests: the temporary project,
//! package inputs, bound output sessions and generated-source projects.

// The embedded Omega programs use `format!` only to keep their many source
// braces visually paired; spelling each as a Rust raw string would require a
// second, error-prone fixture representation.
#![allow(clippy::useless_format)]

#[path = "build_config_granted/checkpoints_and_snapshots.rs"]
mod checkpoints_and_snapshots;
#[path = "build_config_granted/generated_constants.rs"]
mod generated_constants;
#[path = "build_config_granted/generated_invocations.rs"]
mod generated_invocations;
#[path = "build_config_granted/generated_sources_and_authority.rs"]
mod generated_sources_and_authority;

use checked_interpreter::FilesystemSponsor;
use compiler::compile_to_checked;
use package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use semantic_vocabulary::PackageKeyIdentity;
use std::path::{Path, PathBuf};

struct Project {
    root: PathBuf,
}

impl Project {
    fn new(label: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("omega-build-facet-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).expect("create build-facet project");
        Self { root }
    }

    fn write(&self, relative: &str, source: &str) {
        std::fs::write(self.root.join(relative), source).expect("write build-facet fixture");
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
    let package = PackageKeyIdentity::from_digest([97; 32]).expect("nonzero package identity");
    PackageCompilationInputs::new_package(
        package,
        vec![
            PackageSourceBinding::new(package, "build-facet", root.to_path_buf())
                .with_canonical_source_metadata()
                .expect("capture canonical package source"),
        ],
        Vec::new(),
    )
    .expect("single-package build-facet input")
}

fn bound_build_output_session(label: &str) -> (PathBuf, FilesystemSponsor, PathBuf) {
    let session = std::env::temp_dir().join(format!(
        "omega-build-snapshot-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&session);
    std::fs::create_dir(&session).expect("create build snapshot session");
    let session = std::fs::canonicalize(session).expect("canonicalize build snapshot session");
    let sponsor = FilesystemSponsor::new(&session).expect("create build snapshot sponsor");
    let build_dir = session.join("output");
    let bound_build_dir = sponsor
        .bind_path(&build_dir)
        .expect("bind build snapshot output root");
    let prepared_build_dir = sponsor
        .prepare_create_directory(&bound_build_dir)
        .expect("prepare build snapshot output root");
    std::fs::create_dir(&build_dir).expect("create build snapshot output root");
    prepared_build_dir
        .commit()
        .expect("commit build snapshot output root");
    (session, sponsor, build_dir)
}

/// Interleaved output descriptors preserve operation order and hand generated
/// source into the later checking stratum.
fn write_interleaved_build_project(project: &Project) {
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("build-facet-interleaved-build");
    let generated: BuildPath = builder.output.resolve("generated.omg");
    let artifact: BuildPath = builder.output.resolve("artifact.txt");
    let generated_descriptor: i32 = builder.output.create(generated, 438);
    let artifact_descriptor: i32 = builder.output.create(artifact, 438);
    let generated_prefix: i64 = builder.output.write(generated_descriptor, "data BuildGenerated {");
    let artifact_prefix: i64 = builder.output.write(artifact_descriptor, "ab");
    let generated_suffix: i64 = builder.output.write(generated_descriptor, " base: Main; }\n");
    let artifact_middle: i64 = builder.output.write(artifact_descriptor, "Z");
    let generated_newline: i64 = builder.output.write(generated_descriptor, "\n");
    let artifact_suffix: i64 = builder.output.write(artifact_descriptor, "cd");
    let artifact_close: i32 = builder.output.close(artifact_descriptor);
    let generated_close: i32 = builder.output.close(generated_descriptor);
    builder.output.include_source(generated);
}
"#,
    );
}

fn write_mixed_interleaved_build_project(project: &Project) {
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write("prefix.txt", "data BuildGenerated {");
    project.write("suffix.txt", " base: Main; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("build-facet-mixed-interleaved-build");
    let prefix: BuildPath = builder.source.resolve("prefix.txt");
    let suffix: BuildPath = builder.source.resolve("suffix.txt");
    let generated: BuildPath = builder.output.resolve("generated.omg");
    let artifact: BuildPath = builder.output.resolve("artifact.txt");
    let prefix_descriptor: i32 = builder.source.open(prefix, 0);
    let generated_descriptor: i32 = builder.output.create(generated, 438);
    let suffix_descriptor: i32 = builder.source.open(suffix, 0);
    let artifact_descriptor: i32 = builder.output.create(artifact, 438);
    let mut prefix_bytes: [u8; 21];
    let prefix_count: i64 = builder.source.read(prefix_descriptor, &mut prefix_bytes, 21);
    let generated_prefix: i64 = builder.output.write(generated_descriptor, &prefix_bytes);
    let artifact_prefix: i64 = builder.output.write(artifact_descriptor, "ab");
    let mut suffix_bytes: [u8; 15];
    let suffix_count: i64 = builder.source.read(suffix_descriptor, &mut suffix_bytes, 15);
    let generated_suffix: i64 = builder.output.write(generated_descriptor, &suffix_bytes);
    let artifact_middle: i64 = builder.output.write(artifact_descriptor, "Z");
    let generated_newline: i64 = builder.output.write(generated_descriptor, "\n");
    let artifact_suffix: i64 = builder.output.write(artifact_descriptor, "cd");
    let artifact_close: i32 = builder.output.close(artifact_descriptor);
    let generated_close: i32 = builder.output.close(generated_descriptor);
    builder.output.include_source(generated);
    let prefix_end: i64 = builder.source.read(prefix_descriptor, &mut prefix_bytes, 21);
    let suffix_close: i32 = builder.source.close(suffix_descriptor);
    let prefix_close: i32 = builder.source.close(prefix_descriptor);
}
"#,
    );
}

fn sponsored_build_session(label: &str) -> (PathBuf, FilesystemSponsor, PathBuf) {
    let session = std::env::temp_dir().join(format!(
        "omega-build-facet-{label}-session-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&session);
    std::fs::create_dir(&session).expect("create generated-source build session");
    let session = std::fs::canonicalize(session).expect("canonicalize generated-source session");
    let sponsor = FilesystemSponsor::new(&session).expect("create generated-source sponsor");
    let build_dir = session.join("output");
    let bound_build_dir = sponsor
        .bind_path(&build_dir)
        .expect("bind generated-source output root");
    let prepared_build_dir = sponsor
        .prepare_create_directory(&bound_build_dir)
        .expect("prepare generated-source output root");
    std::fs::create_dir(&build_dir).expect("create generated-source output root");
    prepared_build_dir
        .commit()
        .expect("commit generated-source output root");
    (session, sponsor, build_dir)
}
