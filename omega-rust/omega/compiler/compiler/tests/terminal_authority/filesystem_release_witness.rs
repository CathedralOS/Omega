//! How far an ordinary compile carries retained filesystem release evidence.
//!
//! `native_product/realization.rs` captures this compile's own verified build
//! filesystem replay record, derives one ordinary-release contract per
//! retained bounded open/query/close occurrence, and merges the resulting
//! evidence-bound explicit-empty mechanism rows into the terminal-authority
//! policy. The occurrence that derivation needs is the constrained
//! `open_path_handle` / `final_path_name_by_handle` / `close_handle` chain on
//! a Source-rooted path
//! (`build-evaluation/src/evidence/replay_eligibility.rs`,
//! `source_native_query_open_identity`).
//!
//! No authored build machine can request that chain. The compiler-owned build
//! vocabulary exposes `BuildSource::{resolve, open, read, close}` and
//! `BuildOutput::{resolve, create, write, close, ...}` only
//! (`source-files-to-assembled-syntax/src/source_assembly.rs`, `BUILD_PRELUDE`;
//! `checked-interpreter/src/interpreter/evaluator/build_paths.rs`,
//! `build_facet_filesystem_operation`), and reaching the raw `FilesystemHost`
//! boundary from `build.omg` is refused outright by
//! `build-evaluation/src/admitted_build_program.rs` with "build.omg may not
//! reach runtime boundary services; use the compiler-owned Build facets".
//!
//! So the production route stops before the row: the record is real and
//! verified, and it carries no release occurrence. This test pins that
//! boundary rather than hand-building the contract the policy would need.

use checked_interpreter::FilesystemSponsor;
use compiler::{CheckedCompileRequest, compile_to_checked};
use package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use semantic_vocabulary::PackageKeyIdentity;
use std::path::{Path, PathBuf};

struct Project {
    root: PathBuf,
}

impl Project {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-terminal-authority-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).expect("create terminal-authority project");
        Self { root }
    }

    fn write(&self, relative: &str, source: &str) {
        std::fs::write(self.root.join(relative), source).expect("write terminal-authority fixture");
    }

    fn main(&self) -> PathBuf {
        self.root.join("main.omg")
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        set_source_tree_permissions(&self.root, false);
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[cfg(unix)]
fn set_source_tree_permissions(root: &Path, sealed: bool) {
    use std::os::unix::fs::PermissionsExt;

    let metadata = std::fs::symlink_metadata(root).expect("inspect package source fixture");
    if metadata.is_dir() {
        if !sealed {
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o755))
                .expect("unseal package source directory");
        }
        for entry in std::fs::read_dir(root).expect("enumerate package source fixture") {
            set_source_tree_permissions(&entry.expect("read package source entry").path(), sealed);
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
fn set_source_tree_permissions(_root: &Path, _sealed: bool) {}

fn package_inputs(root: &Path) -> PackageCompilationInputs {
    let package = PackageKeyIdentity::from_digest([83; 32]).expect("nonzero package identity");
    PackageCompilationInputs::new_package(
        package,
        vec![
            PackageSourceBinding::new(package, "terminal-authority", root.to_path_buf())
                .with_canonical_source_metadata()
                .expect("capture canonical package source"),
        ],
        Vec::new(),
    )
    .expect("single-package terminal-authority input")
}

fn sponsored_build_session(label: &str) -> (PathBuf, FilesystemSponsor, PathBuf) {
    let session = std::env::temp_dir().join(format!(
        "omega-terminal-authority-{label}-session-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&session);
    std::fs::create_dir(&session).expect("create terminal-authority build session");
    let session = std::fs::canonicalize(session).expect("canonicalize terminal-authority session");
    let sponsor = FilesystemSponsor::new(&session).expect("create terminal-authority sponsor");
    let build_dir = session.join("output");
    let bound_build_dir = sponsor
        .bind_path(&build_dir)
        .expect("bind terminal-authority output root");
    let prepared_build_dir = sponsor
        .prepare_create_directory(&bound_build_dir)
        .expect("prepare terminal-authority output root");
    std::fs::create_dir(&build_dir).expect("create terminal-authority output root");
    prepared_build_dir
        .commit()
        .expect("commit terminal-authority output root");
    (session, sponsor, build_dir)
}

/// The widest filesystem lifecycle an authored build machine can request: the
/// Source facet's `resolve`/`open`/`read`/`close`.
fn write_source_reading_build(project: &Project) {
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write("input.txt", "input\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.package("terminal-authority-release-witness");
    let input: BuildPath = builder.source.resolve("input.txt");
    let descriptor: i32 = builder.source.open(input, 0);
    let mut bytes: [u8; 6];
    let count: i64 = builder.source.read(descriptor, &mut bytes, 6);
    let closed: i32 = builder.source.close(descriptor);
}
"#,
    );
}

/// The constrained acquisition tag the bounded release proof requires.
const OPEN_PATH_HANDLE_TAG: u16 = 28;

#[test]
fn an_ordinary_compile_retains_a_verified_record_without_a_release_occurrence() {
    let project = Project::new("release-witness");
    write_source_reading_build(&project);
    let (session, sponsor, build_dir) = sponsored_build_session("release-witness");
    set_source_tree_permissions(&project.root, true);
    let checked = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(build_dir),
        package_inputs: Some(package_inputs(&project.root)),
        filesystem_sponsor: Some(sponsor),
        ..CheckedCompileRequest::new(&project.main(), Some("linux_x86_64"))
    })
    .expect("the granted build activation executes its Source read chain");
    set_source_tree_permissions(&project.root, false);

    let limits = build_evaluation::BuildFilesystemReplayRecordLimits::default();
    let summary = checked
        .build_observation_summary()
        .expect("the granted activation retains observation custody");
    // The retained evidence is substantive: the build really did acquire,
    // read and release a Source object.
    assert!(
        summary.filesystem_operation_attempts().len() >= 3,
        "the authored Source read chain retains its open, read and close attempts"
    );
    assert!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .all(|attempt| attempt.operation_tag() != OPEN_PATH_HANDLE_TAG),
        "no authored build operation issues the constrained `open_path_handle` acquisition"
    );

    let record = build_evaluation::capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("the verified Source read chain encodes its replay record")
        .expect("a receipted Source-input activation issues a replay record");

    // This is exactly the record and call `native_product/realization.rs`
    // makes. An empty result leaves the terminal-authority policy unchanged,
    // so the demanded release-cohort mechanism keeps its unconstrained key
    // and earns no row.
    let contracts =
        native_realization::filesystem_native_handle_query_release_contracts(&record, limits)
            .expect("the retained record rehydrates");
    assert!(
        contracts.is_empty(),
        "the build vocabulary offers no constrained query-release route, so an ordinary \
         compile derives no occurrence-specific ordinary-release contract"
    );

    let _ = std::fs::remove_dir_all(session);
}
