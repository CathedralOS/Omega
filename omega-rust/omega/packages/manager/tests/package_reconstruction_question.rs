//! Fixtures shared by the package reconstruction question tests: workspace
//! roots, closures, questions and the frame helpers.

#[path = "support/accepted_policy.rs"]
mod accepted_policy_fixture;
#[path = "package_reconstruction_question/association_paths_and_questions.rs"]
mod association_paths_and_questions;
#[path = "package_reconstruction_question/dependency_claims_and_recovery.rs"]
mod dependency_claims_and_recovery;

use package_manager::resolution::graph::{
    PackageSourceClosureLimits, ResolveWorkspacePackageClosureError, ResolvedPackageSourceClosure,
    resolve_external_local_package_closure, resolve_workspace_package_closure,
};
use package_manager::resolution::source::ResolvePackageSourceError;
use package_manager::review::SemanticBindingReview;
use package_manager::review::{
    CanonicalPackageReconstructionQuestion, CanonicalPackageReconstructionQuestionLimits,
    compile_resolved_package_reviews,
};
use package_source::PrimaryGitChoices;
use package_source::{
    ExternalSourceContext, LocalSourceLimits, SourceLineage, SourceRelativePath,
    SourceResolverStorage,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const QUESTION_MAGIC: &[u8] = b"OMEGA-PACKAGE-RECONSTRUCTION-QUESTION\0";

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|ancestor| ancestor.join("tests/fixtures/packages").is_dir())
        .expect("package-manager should live beneath the Omega workspace")
        .to_path_buf()
}

fn temporary_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-package-reconstruction-question-{label}-{}-{}",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed),
    ))
}

fn resolve_workspace_package_closure_from_hardened_base(
    workspace_root_source: &SourceLineage,
    root_member_path: SourceRelativePath,
    live_workspace_root: impl AsRef<Path>,
    cache_dir: impl AsRef<Path>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveWorkspacePackageClosureError> {
    let storage = SourceResolverStorage::for_hardened_base(cache_dir, PrimaryGitChoices::default())
        .map_err(|error| {
            ResolveWorkspacePackageClosureError::Root(ResolvePackageSourceError::Source(error))
        })?;
    resolve_workspace_package_closure(
        workspace_root_source,
        root_member_path,
        live_workspace_root,
        &storage,
        source_limits,
        closure_limits,
    )
}

fn resolve_external_closure(
    live_root: impl AsRef<Path>,
    cache_dir: impl AsRef<Path>,
) -> ResolvedPackageSourceClosure {
    let storage = SourceResolverStorage::for_hardened_base(cache_dir, PrimaryGitChoices::default())
        .expect("source storage");
    resolve_external_local_package_closure(
        live_root,
        ExternalSourceContext::derive(b"open-claim-composition"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve external package closure")
}

fn graph_workbench_question() -> (
    PathBuf,
    ResolvedPackageSourceClosure,
    package_manager::review::CompilerIssuedPackageReviewSet,
    CanonicalPackageReconstructionQuestion,
) {
    let temporary = temporary_root("graph-workbench");
    std::fs::create_dir_all(&temporary).expect("create temporary root");
    let fixture_root = workspace_root().join("tests/fixtures/packages");
    let workspace_lineage = SourceLineage::git("https://github.com/CathedralOS/Omega.git").unwrap();
    let closure = resolve_workspace_package_closure_from_hardened_base(
        &workspace_lineage,
        SourceRelativePath::parse("graph-workbench").unwrap(),
        &fixture_root,
        temporary.join("cache"),
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve graph-workbench source closure");
    let reviews = compile_resolved_package_reviews(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &temporary.join("build"),
        SemanticBindingReview::Discover,
    )
    .expect("compile graph-workbench package reviews");
    let question = CanonicalPackageReconstructionQuestion::from_resolved_and_reviews(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &reviews,
        CanonicalPackageReconstructionQuestionLimits::default(),
    )
    .expect("associate source and obligation closure");
    (temporary, closure, reviews, question)
}

fn claim_free_review_fixture(
    label: &str,
) -> (
    PathBuf,
    ResolvedPackageSourceClosure,
    package_manager::review::CompilerIssuedPackageReviewSet,
) {
    let temporary = temporary_root(label);
    let root = temporary.join("root");
    std::fs::create_dir_all(&root).expect("create claim-free review root");
    std::fs::write(
        root.join("build.omg"),
        "machine build(builder: &mut Build) { builder.package(\"association-canary\"); }\n",
    )
    .expect("write package declaration");
    std::fs::write(root.join("main.omg"), "pub machine value() -> u64 { 1 }\n")
        .expect("write reviewed source");
    let closure = resolve_external_closure(&root, temporary.join("cache"));
    let reviews = compile_resolved_package_reviews(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &temporary.join("build"),
        SemanticBindingReview::Explicit(&[]),
    )
    .expect("compile source-only review");
    (temporary, closure, reviews)
}

fn split_question(bytes: &[u8]) -> (u16, Vec<u8>, Vec<Vec<u8>>) {
    let mut offset = 0usize;
    assert_eq!(&bytes[..QUESTION_MAGIC.len()], QUESTION_MAGIC);
    offset += QUESTION_MAGIC.len();
    let version = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
    offset += 2;
    let source = take_frame(bytes, &mut offset).to_vec();
    assert_eq!(version, 3);
    let entry_count = take_u32(bytes, &mut offset) as usize;
    let entries = (0..entry_count)
        .map(|_| {
            // Keep the checked context attached when testing row removal,
            // replacement or reordering: purpose, target, execution, ledger.
            let start = offset;
            offset += 2;
            take_frame(bytes, &mut offset);
            take_frame(bytes, &mut offset);
            take_frame(bytes, &mut offset);
            bytes[start..offset].to_vec()
        })
        .collect::<Vec<_>>();
    assert_eq!(offset, bytes.len());
    (version, source, entries)
}

fn join_question(version: u16, source: &[u8], entries: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(QUESTION_MAGIC);
    bytes.extend_from_slice(&version.to_le_bytes());
    push_frame(&mut bytes, source);
    bytes.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for entry in entries {
        bytes.extend_from_slice(entry);
    }
    bytes
}

fn take_u32(bytes: &[u8], offset: &mut usize) -> u32 {
    let value = u32::from_le_bytes(bytes[*offset..*offset + 4].try_into().unwrap());
    *offset += 4;
    value
}

fn take_frame<'bytes>(bytes: &'bytes [u8], offset: &mut usize) -> &'bytes [u8] {
    let length = take_u32(bytes, offset) as usize;
    let framed = &bytes[*offset..*offset + length];
    *offset += length;
    framed
}

fn push_frame(bytes: &mut Vec<u8>, framed: &[u8]) {
    bytes.extend_from_slice(&(framed.len() as u32).to_le_bytes());
    bytes.extend_from_slice(framed);
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|candidate| candidate == needle)
}

fn remove_temporary_tree(root: &std::path::Path) {
    make_tree_owner_writable(root);
    std::fs::remove_dir_all(root).expect("remove temporary root");
}

#[cfg(unix)]
fn make_tree_owner_writable(root: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    let Ok(metadata) = std::fs::symlink_metadata(root) else {
        return;
    };
    if !metadata.is_dir() {
        return;
    }
    let mode = metadata.permissions().mode() | 0o700;
    let _ = std::fs::set_permissions(root, std::fs::Permissions::from_mode(mode));
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            make_tree_owner_writable(&entry.path());
        }
    }
}

#[cfg(windows)]
// Test cleanup must undo readonly snapshots before removing their directory.
#[allow(clippy::permissions_set_readonly_false)]
fn make_tree_owner_writable(root: &std::path::Path) {
    let Ok(metadata) = std::fs::symlink_metadata(root) else {
        return;
    };
    let mut permissions = metadata.permissions();
    permissions.set_readonly(false);
    let _ = std::fs::set_permissions(root, permissions);
    if metadata.is_dir()
        && let Ok(entries) = std::fs::read_dir(root)
    {
        for entry in entries.flatten() {
            make_tree_owner_writable(&entry.path());
        }
    }
}
