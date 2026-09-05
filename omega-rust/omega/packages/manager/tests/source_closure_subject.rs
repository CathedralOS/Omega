use package_manager::resolution::graph::{
    CanonicalDependencySourceRequest, CanonicalRootSourceRequest, CanonicalSourceClosureSubject,
    CanonicalSourceClosureSubjectLimits, PackageSourceClosureLimits,
    ResolveExternalLocalPackageClosureError, ResolveWorkspacePackageClosureError,
    ResolvedPackageSourceClosure, resolve_external_local_package_closure_with_storage,
    resolve_workspace_package_closure_with_storage,
};
use package_manager::resolution::source::ResolvePackageSourceError;
use package_source::{
    ExternalSourceContext, LocalSourceLimits, SourceLineage, SourceRelativePath,
    SourceResolverStorage,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TempTree(PathBuf);

impl TempTree {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock follows the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "omega-source-closure-subject-{}-{stamp}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).expect("create source-closure subject fixture root");
        Self(root)
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_package(root: &Path, name: &str, dependency_statements: &str) {
    fs::create_dir_all(root).expect("create package fixture");
    fs::write(
        root.join("build.omg"),
        format!(
            "machine build(builder: &mut Build) {{\n    builder.package(\"{name}\");\n{dependency_statements}}}\n"
        ),
    )
    .expect("write package build declaration");
    fs::write(root.join("main.omg"), "machine root() {}\n").expect("write package source");
}

fn write_diamond(tree: &TempTree) -> PathBuf {
    let sources = tree.path("sources");
    write_package(
        &sources.join("root"),
        "root-package",
        concat!(
            "    builder.depend(Source::Path { location: \"../left\" });\n",
            "    builder.depend_as(\"right_branch\", Source::Path { location: \"../right\" });\n",
        ),
    );
    write_package(
        &sources.join("left"),
        "left-package",
        "    builder.depend(Source::Path { location: \"../shared\" });\n",
    );
    write_package(
        &sources.join("right"),
        "right-package",
        "    builder.depend_as(\"shared_override\", Source::Path { location: \"../right/../shared\" });\n",
    );
    write_package(&sources.join("shared"), "shared-package", "");
    sources.join("root")
}

fn resolve_external_local_package_closure(
    live_root: impl AsRef<Path>,
    source_context: ExternalSourceContext,
    cache_dir: impl AsRef<Path>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    let storage = SourceResolverStorage::for_hardened_base(cache_dir).map_err(|error| {
        ResolveExternalLocalPackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    resolve_external_local_package_closure_with_storage(
        live_root,
        source_context,
        &storage,
        source_limits,
        closure_limits,
    )
}

fn resolve_workspace_package_closure(
    workspace_root_source: &SourceLineage,
    root_member_path: SourceRelativePath,
    live_workspace_root: impl AsRef<Path>,
    cache_dir: impl AsRef<Path>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveWorkspacePackageClosureError> {
    let storage = SourceResolverStorage::for_hardened_base(cache_dir).map_err(|error| {
        ResolveWorkspacePackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    resolve_workspace_package_closure_with_storage(
        workspace_root_source,
        root_member_path,
        live_workspace_root,
        &storage,
        source_limits,
        closure_limits,
    )
}

fn resolve_diamond(
    requested_root: &Path,
    context: ExternalSourceContext,
    cache: &Path,
) -> ResolvedPackageSourceClosure {
    resolve_external_local_package_closure(
        requested_root,
        context,
        cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve real external-local diamond closure")
}

fn path_request(request: &CanonicalDependencySourceRequest) -> (Option<&str>, &str) {
    match request {
        CanonicalDependencySourceRequest::Path {
            explicit_alias,
            location,
        } => (
            explicit_alias.as_ref().map(|alias| alias.as_str()),
            location,
        ),
        CanonicalDependencySourceRequest::Git { .. } => {
            panic!("external-local diamond should contain only path requests")
        }
    }
}

#[test]
fn real_diamond_preserves_each_request_alias_and_canonical_reconstruction() {
    let tree = TempTree::new();
    let root = write_diamond(&tree);
    let context = ExternalSourceContext::derive(b"source-closure-subject-diamond");
    let closure = resolve_diamond(&root, context.clone(), &tree.path("cache-a"));
    let limits = CanonicalSourceClosureSubjectLimits::default();
    let subject = CanonicalSourceClosureSubject::from_resolved(
        &closure.for_exact_target(target::TargetProfile::CrossPlatformCli),
        limits,
    )
    .expect("project canonical source-closure subject");

    assert_eq!(subject.packages().len(), 4);
    assert_eq!(subject.dependency_requests().len(), 4);

    let root_requests = subject
        .dependency_requests()
        .iter()
        .filter(|request| request.requester().name().as_str() == "root-package")
        .collect::<Vec<_>>();
    assert_eq!(root_requests.len(), 2);
    assert_eq!(root_requests[0].dependency_index(), 0);
    assert_eq!(path_request(root_requests[0].request()), (None, "../left"));
    assert_eq!(root_requests[0].alias().as_str(), "left_package");
    assert_eq!(root_requests[1].dependency_index(), 1);
    assert_eq!(
        path_request(root_requests[1].request()),
        (Some("right_branch"), "../right")
    );
    assert_eq!(root_requests[1].alias().as_str(), "right_branch");

    let shared_requests = subject
        .dependency_requests()
        .iter()
        .filter(|request| request.selected().key().name().as_str() == "shared-package")
        .collect::<Vec<_>>();
    assert_eq!(
        shared_requests.len(),
        2,
        "diamond convergence must not collapse either authored request"
    );
    let left_request = shared_requests
        .iter()
        .find(|request| request.requester().name().as_str() == "left-package")
        .expect("left requester occurrence");
    assert_eq!(left_request.dependency_index(), 0);
    assert_eq!(path_request(left_request.request()), (None, "../shared"));
    assert_eq!(left_request.alias().as_str(), "shared_package");
    let right_request = shared_requests
        .iter()
        .find(|request| request.requester().name().as_str() == "right-package")
        .expect("right requester occurrence");
    assert_eq!(right_request.dependency_index(), 0);
    assert_eq!(
        path_request(right_request.request()),
        (Some("shared_override"), "../right/../shared")
    );
    assert_eq!(right_request.alias().as_str(), "shared_override");
    assert_eq!(left_request.selected(), right_request.selected());

    let recovered = CanonicalSourceClosureSubject::recover(subject.canonical_bytes(), limits)
        .expect("recover canonical subject");
    assert_eq!(recovered, subject);
    assert_eq!(recovered.canonical_bytes(), subject.canonical_bytes());
    assert_eq!(recovered.fingerprint(), subject.fingerprint());
    assert!(
        recovered
            .matches_resolved(
                &closure.for_exact_target(target::TargetProfile::CrossPlatformCli),
                limits,
            )
            .expect("reconstruct subject from resolver custody")
    );

    let relocated = resolve_diamond(&root, context, &tree.path("cache-b"));
    let relocated_subject = CanonicalSourceClosureSubject::from_resolved(
        &relocated.for_exact_target(target::TargetProfile::CrossPlatformCli),
        limits,
    )
    .expect("project subject after cache relocation");
    assert_eq!(
        relocated_subject, subject,
        "snapshot/cache custody paths must not enter the canonical subject"
    );
}

#[test]
fn exact_root_spelling_changes_the_subject_without_changing_selected_identity() {
    let tree = TempTree::new();
    let root = write_diamond(&tree);
    let alternate_spelling = root.join("..").join("root");
    let context = ExternalSourceContext::derive(b"source-closure-subject-root-spelling");
    let limits = CanonicalSourceClosureSubjectLimits::default();

    let ordinary = resolve_diamond(&root, context.clone(), &tree.path("ordinary-cache"));
    let alternate = resolve_diamond(&alternate_spelling, context, &tree.path("alternate-cache"));
    let ordinary_subject = CanonicalSourceClosureSubject::from_resolved(
        &ordinary.for_exact_target(target::TargetProfile::CrossPlatformCli),
        limits,
    )
    .expect("project ordinary-spelling subject");
    let alternate_subject = CanonicalSourceClosureSubject::from_resolved(
        &alternate.for_exact_target(target::TargetProfile::CrossPlatformCli),
        limits,
    )
    .expect("project alternate-spelling subject");

    assert_eq!(
        ordinary_subject.root().selected(),
        alternate_subject.root().selected()
    );
    assert_eq!(ordinary_subject.packages(), alternate_subject.packages());
    assert_eq!(
        ordinary_subject.dependency_requests(),
        alternate_subject.dependency_requests()
    );
    let (
        CanonicalRootSourceRequest::ExternalLocal {
            requested_root: ordinary_request,
            ..
        },
        CanonicalRootSourceRequest::ExternalLocal {
            requested_root: alternate_request,
            ..
        },
    ) = (
        ordinary_subject.root().request(),
        alternate_subject.root().request(),
    )
    else {
        panic!("fixture roots should retain external-local requests")
    };
    assert_ne!(ordinary_request, alternate_request);
    assert_ne!(ordinary_subject, alternate_subject);
    assert_ne!(
        ordinary_subject.fingerprint(),
        alternate_subject.fingerprint()
    );
    assert!(
        !ordinary_subject
            .matches_resolved(
                &alternate.for_exact_target(target::TargetProfile::CrossPlatformCli),
                limits,
            )
            .expect("compare exact alternate request")
    );
    assert!(
        !alternate_subject
            .matches_resolved(
                &ordinary.for_exact_target(target::TargetProfile::CrossPlatformCli),
                limits,
            )
            .expect("compare exact ordinary request")
    );
}

#[test]
fn recovery_rejects_bounds_trailing_bytes_and_semantic_tampering() {
    let tree = TempTree::new();
    let root = write_diamond(&tree);
    let closure = resolve_diamond(
        &root,
        ExternalSourceContext::derive(b"source-closure-subject-recovery"),
        &tree.path("cache"),
    );
    let limits = CanonicalSourceClosureSubjectLimits::default();
    let subject = CanonicalSourceClosureSubject::from_resolved(
        &closure.for_exact_target(target::TargetProfile::CrossPlatformCli),
        limits,
    )
    .expect("project recovery fixture subject");
    let bytes = subject.canonical_bytes();

    let record_bound = CanonicalSourceClosureSubjectLimits {
        maximum_record_bytes: bytes.len() - 1,
        ..limits
    };
    assert_eq!(
        CanonicalSourceClosureSubject::recover(bytes, record_bound)
            .expect_err("record over configured bound must reject")
            .message(),
        "source-closure subject exceeds its record-byte limit"
    );

    let package_bound = CanonicalSourceClosureSubjectLimits {
        maximum_packages: subject.packages().len() - 1,
        ..limits
    };
    assert!(
        CanonicalSourceClosureSubject::recover(bytes, package_bound).is_err(),
        "decoded package count must remain bounded"
    );

    let request_bound = CanonicalSourceClosureSubjectLimits {
        maximum_dependency_requests: subject.dependency_requests().len() - 1,
        ..limits
    };
    assert!(
        CanonicalSourceClosureSubject::recover(bytes, request_bound).is_err(),
        "decoded request-occurrence count must remain bounded"
    );

    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert_eq!(
        CanonicalSourceClosureSubject::recover(&trailing, limits)
            .expect_err("trailing bytes must reject")
            .message(),
        "source-closure subject has trailing bytes"
    );

    let mut damaged_magic = bytes.to_vec();
    damaged_magic[0] ^= 1;
    assert!(
        CanonicalSourceClosureSubject::recover(&damaged_magic, limits).is_err(),
        "framing tampering must reject"
    );

    let alias = b"shared_override";
    let alias_offsets = bytes
        .windows(alias.len())
        .enumerate()
        .filter_map(|(offset, candidate)| (candidate == alias).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(
        alias_offsets.len(),
        3,
        "projected, selected, and resolved aliases are encoded independently"
    );
    let mut inconsistent_alias = bytes.to_vec();
    inconsistent_alias[alias_offsets[2]] = b'x';
    assert_eq!(
        CanonicalSourceClosureSubject::recover(&inconsistent_alias, limits)
            .expect_err("request/edge alias disagreement must reject")
            .message(),
        "dependency request alias disagrees with its authored selection"
    );
}

#[test]
fn workspace_member_requests_round_trip_without_cache_identity() {
    let tree = TempTree::new();
    let workspace = tree.path("workspace");
    write_package(
        &workspace.join("root"),
        "workspace-root",
        "    builder.depend(Source::Path { location: \"../child\" });\n",
    );
    write_package(&workspace.join("child"), "workspace-child", "");
    let workspace_source =
        SourceLineage::git("https://github.com/CathedralOS/source-subject-workspace.git")
            .expect("canonical workspace source identity");
    let limits = CanonicalSourceClosureSubjectLimits::default();
    let closure = resolve_workspace_package_closure(
        &workspace_source,
        SourceRelativePath::parse("root").expect("root member path"),
        &workspace,
        tree.path("workspace-cache-a"),
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve workspace member closure");
    let subject = CanonicalSourceClosureSubject::from_resolved(
        &closure.for_exact_target(target::TargetProfile::CrossPlatformCli),
        limits,
    )
    .expect("project workspace source subject");
    assert!(matches!(
        subject.root().request(),
        CanonicalRootSourceRequest::WorkspaceMember {
            workspace_root_source,
            member_path,
            ..
        } if workspace_root_source == &workspace_source && member_path.as_str() == "root"
    ));
    assert_eq!(subject.packages().len(), 2);
    assert_eq!(subject.dependency_requests().len(), 1);
    assert_eq!(
        CanonicalSourceClosureSubject::recover(subject.canonical_bytes(), limits)
            .expect("recover workspace source subject"),
        subject
    );

    let relocated = resolve_workspace_package_closure(
        &workspace_source,
        SourceRelativePath::parse("root").expect("root member path"),
        &workspace,
        tree.path("workspace-cache-b"),
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve workspace member closure after cache relocation");
    assert_eq!(
        CanonicalSourceClosureSubject::from_resolved(
            &relocated.for_exact_target(target::TargetProfile::CrossPlatformCli),
            limits,
        )
        .expect("project relocated workspace subject"),
        subject
    );
}
