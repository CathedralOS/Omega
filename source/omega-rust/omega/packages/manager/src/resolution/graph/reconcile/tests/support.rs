use crate::manifest::{AliasName, PackageKey, PackageName};
use crate::manifest::dependencies::read::DependencySourceRequest;
use crate::resolution::source::PackageSourceCustody;
use omega_package_source::{
    GitCommitId, GitSourceRequest, GitTreeId, ImmutableSourceResolution, LocalSourceLimits,
    SourceLineage, SourceRelativePath,
};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn key(name: &str, repository: &str) -> PackageKey {
    PackageKey::new(
        PackageName::parse(name).expect("valid package name"),
        SourceLineage::git(&format!("https://github.com/CathedralOS/{repository}.git"))
            .expect("valid Git lineage"),
    )
}

pub(super) fn resolution(marker: u8) -> ImmutableSourceResolution {
    let commit_digit = char::from_digit(u32::from(marker % 10), 16).unwrap();
    let tree_digit = char::from_digit(u32::from((marker + 1) % 10), 16).unwrap();
    ImmutableSourceResolution::git(
        GitCommitId::parse_hex(&commit_digit.to_string().repeat(40)).unwrap(),
        GitTreeId::parse_hex(&tree_digit.to_string().repeat(40)).unwrap(),
    )
    .unwrap()
}

pub(super) fn request(location: &str) -> DependencySourceRequest {
    DependencySourceRequest::Path {
        explicit_alias: None,
        location: location.to_owned(),
    }
}

pub(super) fn request_as(alias: &str, location: &str) -> DependencySourceRequest {
    DependencySourceRequest::Path {
        explicit_alias: Some(AliasName::parse(alias).expect("valid alias")),
        location: location.to_owned(),
    }
}

pub(super) fn request_location(request: &DependencySourceRequest) -> &str {
    match request {
        DependencySourceRequest::Path { location, .. } => location,
        DependencySourceRequest::Git { repository, .. } => repository,
    }
}

pub(super) fn custody(
    name: &str,
    repository: &str,
    marker: u8,
    snapshot_root: &str,
    dependency_requests: Vec<DependencySourceRequest>,
) -> PackageSourceCustody {
    custody_with_role(
        name,
        repository,
        marker,
        snapshot_root,
        crate::manifest::BuildDeclarationKind::Package,
        dependency_requests,
    )
}

pub(super) fn custody_with_role(
    name: &str,
    repository: &str,
    marker: u8,
    snapshot_root: &str,
    role: crate::manifest::BuildDeclarationKind,
    dependency_requests: Vec<DependencySourceRequest>,
) -> PackageSourceCustody {
    let resolution = resolution(marker);
    let materialization =
        crate::resolution::source::PackageSourceMaterialization::synthetic(resolution.content().clone());
    PackageSourceCustody::from_resolved_parts(
        key(name, repository),
        role,
        resolution,
        materialization,
        PathBuf::from(snapshot_root),
        crate::resolution::source::PackageSourceNavigation::Root,
        crate::resolution::source::PackageSourceSelectionEvidence::Root,
        LocalSourceLimits::default(),
        dependency_requests,
    )
}

pub(super) fn git_root_request(
    root: &PackageSourceCustody,
) -> super::super::PackageRootSourceRequest {
    super::super::PackageRootSourceRequest::Git(crate::resolution::source::GitPackageSourceRequest::root(
        GitSourceRequest::new(
            format!(
                "https://github.com/CathedralOS/{}.git",
                root.key().name().as_str()
            ),
            Some("HEAD".to_owned()),
        )
        .expect("synthetic root request"),
    ))
}

pub(super) fn fake_adapter(
    packages: BTreeMap<&'static str, PackageSourceCustody>,
) -> impl FnMut(
    &PackageSourceCustody,
    &DependencySourceRequest,
) -> Result<PackageSourceCustody, &'static str> {
    move |_, request| {
        packages
            .get(request_location(request))
            .cloned()
            .ok_or("unknown fake source")
    }
}

pub(super) fn package_fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../tests/fixtures/packages")
}

pub(super) fn temp_cache() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time follows Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "omega-package-closure-{}-{stamp}",
        std::process::id()
    ))
}

pub(super) fn workspace_member_request(
    requester: &PackageSourceCustody,
    location: &str,
) -> Result<SourceRelativePath, String> {
    let SourceLineage::Workspace(lineage) = requester.key().source_lineage() else {
        return Err("path requester is not a workspace member".to_owned());
    };
    let mut normalized = PathBuf::from(lineage.member_path().as_str());
    for component in Path::new(location).components() {
        match component {
            Component::Normal(component) => normalized.push(component),
            Component::CurDir => {}
            Component::ParentDir if normalized.pop() => {}
            _ => return Err("path request escapes the fixture workspace".to_owned()),
        }
    }
    SourceRelativePath::parse(
        normalized
            .to_str()
            .ok_or_else(|| "fixture member path is not UTF-8".to_owned())?,
    )
    .map_err(|error| error.to_string())
}
