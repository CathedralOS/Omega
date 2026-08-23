use crate::audit::{PackageGraphAudit, PackageGraphAuditError, audit_package_graph};
use crate::diff::{ManifestDelta, ManifestDiff, diff_package_capability_manifests};
use crate::install::{PackageInstallPlan, PackageInstallPlanError, plan_package_install};
use crate::lock::{PackageLock, PackageLockAssemblyError, PackageLockPersistenceError};
use crate::manifest::{
    AliasName, PackageCapabilityManifest, PackageCapabilityManifestPersistenceError, PackageName,
};
use crate::resolver::{SourceCachePolicyRecord, SourceCacheRequest, resolve_source_cache_record};
use crate::review::{
    CapabilityChangeReceipt, CapabilityChangeReceiptPersistenceError, CapabilityReviewError,
};
use crate::source::{
    GitSourceSpec, LocalSourceLimits, SourceResolveError, resolve_git_source, resolve_local_source,
};
use crate::update::{PackageLockUpdatePlan, PackageLockUpdatePlanError, plan_package_lock_update};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSourceRequest {
    LocalPath(PathBuf),
    Git { url: String, rev: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSourceRequestParseError {
    EmptySourceLocator,
    LocalSourceCannotUseRevision { locator: String, rev: String },
    UnsupportedFileUrl { locator: String },
}

impl PackageSourceRequest {
    pub fn parse(
        locator: impl Into<String>,
        rev: Option<String>,
    ) -> Result<Self, PackageSourceRequestParseError> {
        let locator = locator.into();
        let locator = locator.trim();
        if locator.is_empty() {
            return Err(PackageSourceRequestParseError::EmptySourceLocator);
        }
        if let Some(path) = file_url_path(locator)? {
            reject_local_rev(locator, &rev)?;
            return Ok(Self::LocalPath(path));
        }

        let path = PathBuf::from(locator);
        if is_local_path_locator(locator, &path) {
            reject_local_rev(locator, &rev)?;
            Ok(Self::LocalPath(path))
        } else {
            Ok(Self::Git {
                url: locator.to_owned(),
                rev,
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSourceAudit {
    pub source_kind: String,
    pub locator: String,
    pub requested_rev: Option<String>,
    pub resolved_commit: Option<String>,
    pub resolved_tree: Option<String>,
    pub content_identity: String,
    pub file_count: usize,
    pub byte_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageGraphAuditCommand {
    pub lock_path: PathBuf,
    pub audit: PackageGraphAudit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageLockAssemblyCommand {
    pub out_path: PathBuf,
    pub lock: PackageLock,
    pub audit: PackageGraphAudit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageInstallPlanCommand {
    pub lock_path: PathBuf,
    pub plan: PackageInstallPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageLockUpdatePlanCommand {
    pub lock_path: PathBuf,
    pub receipt_path: Option<PathBuf>,
    pub plan: PackageLockUpdatePlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityChangeReviewCommand {
    pub receipt: CapabilityChangeReceipt,
    pub diff: ManifestDiff,
    pub blocking_deltas: Vec<ManifestDelta>,
}

impl PackageGraphAuditCommand {
    pub fn to_text(&self) -> String {
        let mut report = String::new();
        report.push_str("lock: ");
        report.push_str(&self.lock_path.display().to_string());
        report.push('\n');
        report.push_str(&self.audit.to_text());
        report
    }
}

impl PackageLockAssemblyCommand {
    pub fn to_text(&self) -> String {
        let mut report = String::new();
        report.push_str("package lock assembled\n");
        report.push_str("out: ");
        report.push_str(&self.out_path.display().to_string());
        report.push('\n');
        report.push_str("root package: ");
        report.push_str(self.lock.root_package.as_str());
        report.push('\n');
        report.push_str("lock fingerprint: ");
        report.push_str(&self.lock.fingerprint());
        report.push('\n');
        report.push_str(&self.audit.to_text());
        report
    }
}

impl PackageInstallPlanCommand {
    pub fn to_text(&self) -> String {
        let mut report = String::new();
        report.push_str("lock: ");
        report.push_str(&self.lock_path.display().to_string());
        report.push('\n');
        report.push_str(&self.plan.to_text());
        report
    }
}

impl PackageLockUpdatePlanCommand {
    pub fn to_text(&self) -> String {
        let mut report = String::new();
        report.push_str("lock: ");
        report.push_str(&self.lock_path.display().to_string());
        report.push('\n');
        if let Some(receipt_path) = &self.receipt_path {
            report.push_str("receipt: ");
            report.push_str(&receipt_path.display().to_string());
            report.push('\n');
        }
        report.push_str(&self.plan.to_text());
        report
    }
}

impl CapabilityChangeReviewCommand {
    pub fn to_text(&self) -> String {
        let mut report = String::new();
        report.push_str("capability-change review receipt\n");
        report.push_str("receipt: ");
        report.push_str(&self.receipt.fingerprint());
        report.push('\n');
        report.push_str("old source: ");
        report.push_str(&self.receipt.old_source_identity);
        report.push('\n');
        report.push_str("new source: ");
        report.push_str(&self.receipt.new_source_identity);
        report.push('\n');
        report.push_str("blocking sections:");
        for delta in &self.blocking_deltas {
            report.push(' ');
            report.push_str(&delta.section);
            report.push('(');
            report.push_str(delta.severity.as_str());
            report.push(')');
        }
        report.push('\n');
        report.push_str(&self.diff.to_text());
        report
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageGraphAuditCommandError {
    Lock(PackageLockPersistenceError),
    Graph(PackageGraphAuditError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageGraphAuditFromPathsCommandError {
    Lock(PackageLockPersistenceError),
    Manifest {
        path: PathBuf,
        error: PackageCapabilityManifestPersistenceError,
    },
    Graph(PackageGraphAuditError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageLockAssemblyFromPathsCommandError {
    Manifest {
        path: PathBuf,
        error: PackageCapabilityManifestPersistenceError,
    },
    Assembly(PackageLockAssemblyError),
    Graph(PackageGraphAuditError),
    Write(PackageLockPersistenceError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageInstallPlanCommandError {
    Lock(PackageLockPersistenceError),
    Plan(PackageInstallPlanError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageLockUpdatePlanCommandError {
    Lock(PackageLockPersistenceError),
    Receipt(CapabilityChangeReceiptPersistenceError),
    Plan(PackageLockUpdatePlanError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityChangeReviewCommandError {
    PackageMismatch {
        old_package: String,
        new_package: String,
    },
    NoManifestChange {
        package: String,
    },
    SourceOnlyChange {
        package: String,
    },
    Review(CapabilityReviewError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSourceAuditCommandError {
    Parse(PackageSourceRequestParseError),
    Resolve(SourceResolveError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceCachePolicyCommandError {
    Parse(PackageSourceRequestParseError),
}

impl PackageSourceAudit {
    pub fn to_text(&self) -> String {
        let mut report = String::new();
        report.push_str("package source audit\n");
        report.push_str("source kind: ");
        report.push_str(&self.source_kind);
        report.push('\n');
        report.push_str("locator: ");
        report.push_str(&self.locator);
        report.push('\n');
        if let Some(rev) = &self.requested_rev {
            report.push_str("requested rev: ");
            report.push_str(rev);
            report.push('\n');
        }
        if let Some(commit) = &self.resolved_commit {
            report.push_str("resolved commit: ");
            report.push_str(commit);
            report.push('\n');
        }
        if let Some(tree) = &self.resolved_tree {
            report.push_str("resolved tree: ");
            report.push_str(tree);
            report.push('\n');
        }
        report.push_str("content identity: ");
        report.push_str(&self.content_identity);
        report.push('\n');
        report.push_str("files: ");
        report.push_str(&self.file_count.to_string());
        report.push('\n');
        report.push_str("bytes: ");
        report.push_str(&self.byte_count.to_string());
        report.push('\n');
        report
    }
}

fn file_url_path(locator: &str) -> Result<Option<PathBuf>, PackageSourceRequestParseError> {
    let Some(rest) = locator.strip_prefix("file://") else {
        return Ok(None);
    };
    if !rest.starts_with('/') {
        return Err(PackageSourceRequestParseError::UnsupportedFileUrl {
            locator: locator.to_owned(),
        });
    }
    Ok(Some(PathBuf::from(rest)))
}

fn is_local_path_locator(locator: &str, path: &Path) -> bool {
    path.is_absolute()
        || locator == "."
        || locator == ".."
        || locator.starts_with("./")
        || locator.starts_with("../")
        || path.exists()
}

fn reject_local_rev(
    locator: &str,
    rev: &Option<String>,
) -> Result<(), PackageSourceRequestParseError> {
    if let Some(rev) = rev {
        return Err(
            PackageSourceRequestParseError::LocalSourceCannotUseRevision {
                locator: locator.to_owned(),
                rev: rev.clone(),
            },
        );
    }
    Ok(())
}

pub fn audit_package_source(
    request: PackageSourceRequest,
    cache_dir: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<PackageSourceAudit, SourceResolveError> {
    match request {
        PackageSourceRequest::LocalPath(path) => {
            let resolved = resolve_local_source(&path, limits)?;
            Ok(PackageSourceAudit {
                source_kind: "local-path".to_owned(),
                locator: path.display().to_string(),
                requested_rev: None,
                resolved_commit: None,
                resolved_tree: None,
                content_identity: resolved.content_identity,
                file_count: resolved.file_count,
                byte_count: resolved.byte_count,
            })
        }
        PackageSourceRequest::Git { url, rev } => {
            let resolved = resolve_git_source(
                &GitSourceSpec {
                    url: url.clone(),
                    rev: rev.clone(),
                },
                cache_dir,
                limits,
            )?;
            Ok(PackageSourceAudit {
                source_kind: "git".to_owned(),
                locator: url,
                requested_rev: Some(resolved.requested_rev),
                resolved_commit: Some(resolved.commit),
                resolved_tree: Some(resolved.tree),
                content_identity: resolved.local.content_identity,
                file_count: resolved.local.file_count,
                byte_count: resolved.local.byte_count,
            })
        }
    }
}

pub fn audit_package_source_locator(
    locator: impl Into<String>,
    rev: Option<String>,
    cache_dir: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<PackageSourceAudit, PackageSourceAuditCommandError> {
    let request =
        PackageSourceRequest::parse(locator, rev).map_err(PackageSourceAuditCommandError::Parse)?;
    audit_package_source(request, cache_dir, limits)
        .map_err(PackageSourceAuditCommandError::Resolve)
}

pub fn resolve_source_cache_record_locator(
    locator: impl Into<String>,
    rev: Option<String>,
    cache_dir: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<SourceCachePolicyRecord, SourceCachePolicyCommandError> {
    let request =
        PackageSourceRequest::parse(locator, rev).map_err(SourceCachePolicyCommandError::Parse)?;
    Ok(resolve_source_cache_record(
        source_cache_request_from_package_request(request),
        cache_dir,
        limits,
    ))
}

fn source_cache_request_from_package_request(request: PackageSourceRequest) -> SourceCacheRequest {
    match request {
        PackageSourceRequest::LocalPath(path) => SourceCacheRequest::LocalPath(path),
        PackageSourceRequest::Git { url, rev } => SourceCacheRequest::Git { url, rev },
    }
}

pub fn audit_package_graph_from_lock(
    lock_path: impl AsRef<Path>,
    manifests: &[PackageCapabilityManifest],
) -> Result<PackageGraphAuditCommand, PackageGraphAuditCommandError> {
    let lock_path = lock_path.as_ref();
    let lock =
        PackageLock::read_from_path(lock_path).map_err(PackageGraphAuditCommandError::Lock)?;
    let audit =
        audit_package_graph(&lock, manifests).map_err(PackageGraphAuditCommandError::Graph)?;
    Ok(PackageGraphAuditCommand {
        lock_path: lock_path.to_path_buf(),
        audit,
    })
}

pub fn audit_package_graph_from_paths(
    lock_path: impl AsRef<Path>,
    manifest_paths: &[PathBuf],
) -> Result<PackageGraphAuditCommand, PackageGraphAuditFromPathsCommandError> {
    let lock_path = lock_path.as_ref();
    let lock = PackageLock::read_from_path(lock_path)
        .map_err(PackageGraphAuditFromPathsCommandError::Lock)?;
    let manifests = read_manifest_paths(manifest_paths)?;
    let audit = audit_package_graph(&lock, &manifests)
        .map_err(PackageGraphAuditFromPathsCommandError::Graph)?;
    Ok(PackageGraphAuditCommand {
        lock_path: lock_path.to_path_buf(),
        audit,
    })
}

fn read_manifest_paths(
    manifest_paths: &[PathBuf],
) -> Result<Vec<PackageCapabilityManifest>, PackageGraphAuditFromPathsCommandError> {
    manifest_paths
        .iter()
        .map(|path| {
            PackageCapabilityManifest::read_from_path(path).map_err(|error| {
                PackageGraphAuditFromPathsCommandError::Manifest {
                    path: path.clone(),
                    error,
                }
            })
        })
        .collect()
}

fn read_manifest_paths_for_lock_assembly(
    manifest_paths: &[PathBuf],
) -> Result<Vec<PackageCapabilityManifest>, PackageLockAssemblyFromPathsCommandError> {
    manifest_paths
        .iter()
        .map(|path| {
            PackageCapabilityManifest::read_from_path(path).map_err(|error| {
                PackageLockAssemblyFromPathsCommandError::Manifest {
                    path: path.clone(),
                    error,
                }
            })
        })
        .collect()
}

pub fn assemble_package_lock_from_paths(
    root_package: &PackageName,
    manifest_paths: &[PathBuf],
    out_path: impl AsRef<Path>,
) -> Result<PackageLockAssemblyCommand, PackageLockAssemblyFromPathsCommandError> {
    let out_path = out_path.as_ref();
    let manifests = read_manifest_paths_for_lock_assembly(manifest_paths)?;
    let lock = PackageLock::from_manifests(root_package.clone(), &manifests)
        .map_err(PackageLockAssemblyFromPathsCommandError::Assembly)?;
    let audit = audit_package_graph(&lock, &manifests)
        .map_err(PackageLockAssemblyFromPathsCommandError::Graph)?;
    lock.write_to_path(out_path)
        .map_err(PackageLockAssemblyFromPathsCommandError::Write)?;
    Ok(PackageLockAssemblyCommand {
        out_path: out_path.to_path_buf(),
        lock,
        audit,
    })
}

pub fn plan_package_install_from_lock(
    lock_path: impl AsRef<Path>,
    current_manifests: &[PackageCapabilityManifest],
    candidate_manifests: &[PackageCapabilityManifest],
    dependency_alias: &AliasName,
    dependency_package: &PackageName,
) -> Result<PackageInstallPlanCommand, PackageInstallPlanCommandError> {
    let lock_path = lock_path.as_ref();
    let lock =
        PackageLock::read_from_path(lock_path).map_err(PackageInstallPlanCommandError::Lock)?;
    let plan = plan_package_install(
        &lock,
        current_manifests,
        candidate_manifests,
        dependency_alias,
        dependency_package,
    )
    .map_err(PackageInstallPlanCommandError::Plan)?;
    Ok(PackageInstallPlanCommand {
        lock_path: lock_path.to_path_buf(),
        plan,
    })
}

pub fn plan_package_lock_update_from_lock(
    lock_path: impl AsRef<Path>,
    current_manifests: &[PackageCapabilityManifest],
    candidate_manifests: &[PackageCapabilityManifest],
    target_package: &PackageName,
    receipt_path: Option<&Path>,
) -> Result<PackageLockUpdatePlanCommand, PackageLockUpdatePlanCommandError> {
    let lock_path = lock_path.as_ref();
    let lock =
        PackageLock::read_from_path(lock_path).map_err(PackageLockUpdatePlanCommandError::Lock)?;
    let receipt = receipt_path
        .map(CapabilityChangeReceipt::read_from_path)
        .transpose()
        .map_err(PackageLockUpdatePlanCommandError::Receipt)?;
    let plan = plan_package_lock_update(
        &lock,
        current_manifests,
        candidate_manifests,
        target_package,
        receipt.as_ref(),
    )
    .map_err(PackageLockUpdatePlanCommandError::Plan)?;
    Ok(PackageLockUpdatePlanCommand {
        lock_path: lock_path.to_path_buf(),
        receipt_path: receipt_path.map(Path::to_path_buf),
        plan,
    })
}

pub fn create_capability_change_review(
    old_manifest: &PackageCapabilityManifest,
    new_manifest: &PackageCapabilityManifest,
    reviewer: impl Into<String>,
    reason: impl Into<String>,
) -> Result<CapabilityChangeReviewCommand, CapabilityChangeReviewCommandError> {
    let old = old_manifest.normalized_clone();
    let new = new_manifest.normalized_clone();
    if old.package != new.package {
        return Err(CapabilityChangeReviewCommandError::PackageMismatch {
            old_package: old.package.as_str().to_owned(),
            new_package: new.package.as_str().to_owned(),
        });
    }
    let diff = diff_package_capability_manifests(&old, &new);
    if diff.is_empty() {
        return Err(CapabilityChangeReviewCommandError::NoManifestChange {
            package: old.package.as_str().to_owned(),
        });
    }
    let blocking_deltas = diff
        .deltas
        .iter()
        .filter(|delta| delta.section != "source")
        .cloned()
        .collect::<Vec<_>>();
    if blocking_deltas.is_empty() {
        return Err(CapabilityChangeReviewCommandError::SourceOnlyChange {
            package: old.package.as_str().to_owned(),
        });
    }
    let receipt = CapabilityChangeReceipt::from_diff(
        &diff,
        old.source.resolved,
        new.source.resolved,
        reviewer,
        reason,
    )
    .map_err(CapabilityChangeReviewCommandError::Review)?;

    Ok(CapabilityChangeReviewCommand {
        receipt,
        diff,
        blocking_deltas,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::diff_package_capability_manifests;
    use crate::lock::{LockedDependency, LockedPackage};
    use crate::manifest::{AliasName, DependencyAlias, PackageName, SourceIdentity};
    use std::ffi::OsStr;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "omega-package-command-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn run_test_git<I, S>(directory: &Path, args: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = Command::new("git")
            .current_dir(directory)
            .args(args)
            .output()
            .expect("spawn git");
        assert!(
            output.status.success(),
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn package(name: &str) -> PackageName {
        PackageName::parse(name).unwrap()
    }

    fn alias(name: &str) -> AliasName {
        AliasName::parse(name).unwrap()
    }

    fn dependency(alias: &str, package: &str) -> DependencyAlias {
        DependencyAlias {
            alias: self::alias(alias),
            package: self::package(package),
            source_fingerprint: format!("source:{package}"),
        }
    }

    fn manifest(package: &str) -> PackageCapabilityManifest {
        PackageCapabilityManifest::new(
            PackageName::parse(package).unwrap(),
            SourceIdentity {
                kind: "git".to_owned(),
                locator: format!("https://github.com/CathedralOS/{package}"),
                resolved: format!("commit:{package}"),
            },
        )
    }

    fn lock_from_manifests(manifests: &[PackageCapabilityManifest]) -> PackageLock {
        PackageLock::from_manifests(package("graph-workbench"), manifests)
            .expect("assemble lock fixture")
    }

    fn locked_package(manifest: &PackageCapabilityManifest) -> LockedPackage {
        LockedPackage {
            package: manifest.package.clone(),
            source_kind: manifest.source.kind.clone(),
            source_locator: manifest.source.locator.clone(),
            source_identity: manifest.source.resolved.clone(),
            manifest_fingerprint: manifest.fingerprint(),
            build_observation: manifest.build_machine.observation_class.clone(),
            dependencies: Vec::new(),
            trust_receipts: Vec::new(),
        }
    }

    fn graph_lock(
        root_manifest: &PackageCapabilityManifest,
        child_manifest: &PackageCapabilityManifest,
    ) -> PackageLock {
        let mut root = locked_package(root_manifest);
        root.dependencies.push(LockedDependency {
            alias: alias("file_journal"),
            package: package("file-journal"),
        });
        let mut lock = PackageLock::new(root_manifest.package.clone());
        lock.packages = vec![root, locked_package(child_manifest)];
        lock
    }

    #[test]
    fn source_request_parse_classifies_local_paths_and_file_urls() {
        assert_eq!(
            PackageSourceRequest::parse("./fixtures/packages/file-journal", None),
            Ok(PackageSourceRequest::LocalPath(PathBuf::from(
                "./fixtures/packages/file-journal"
            )))
        );
        assert_eq!(
            PackageSourceRequest::parse("file:///tmp/file-journal", None),
            Ok(PackageSourceRequest::LocalPath(PathBuf::from(
                "/tmp/file-journal"
            )))
        );
    }

    #[test]
    fn source_request_parse_classifies_git_like_locators() {
        assert_eq!(
            PackageSourceRequest::parse(
                "https://github.com/CathedralOS/file-journal",
                Some("fd4ff9824c83a85584661acad93033304512f8c8".to_owned())
            ),
            Ok(PackageSourceRequest::Git {
                url: "https://github.com/CathedralOS/file-journal".to_owned(),
                rev: Some("fd4ff9824c83a85584661acad93033304512f8c8".to_owned()),
            })
        );
        assert_eq!(
            PackageSourceRequest::parse("git@github.com:CathedralOS/file-journal.git", None),
            Ok(PackageSourceRequest::Git {
                url: "git@github.com:CathedralOS/file-journal.git".to_owned(),
                rev: None,
            })
        );
    }

    #[test]
    fn source_request_parse_rejects_empty_or_revisioned_local_sources() {
        assert_eq!(
            PackageSourceRequest::parse("", None),
            Err(PackageSourceRequestParseError::EmptySourceLocator)
        );
        assert_eq!(
            PackageSourceRequest::parse(
                "./fixtures/packages/file-journal",
                Some("HEAD".to_owned())
            ),
            Err(
                PackageSourceRequestParseError::LocalSourceCannotUseRevision {
                    locator: "./fixtures/packages/file-journal".to_owned(),
                    rev: "HEAD".to_owned(),
                }
            )
        );
        assert_eq!(
            PackageSourceRequest::parse("file://relative/path", None),
            Err(PackageSourceRequestParseError::UnsupportedFileUrl {
                locator: "file://relative/path".to_owned(),
            })
        );
    }

    #[test]
    fn local_source_audit_reports_resolved_identity() {
        let root = temp_root("local");
        let cache = temp_root("cache");
        std::fs::create_dir_all(&root).expect("create local package");
        std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");

        let audit = audit_package_source(
            PackageSourceRequest::LocalPath(root.clone()),
            &cache,
            LocalSourceLimits::default(),
        )
        .expect("audit local source");

        assert_eq!(audit.source_kind, "local-path");
        assert_eq!(audit.file_count, 1);
        assert_eq!(audit.content_identity.len(), 64);
        assert!(audit.to_text().contains("package source audit"));

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn source_audit_command_parses_and_audits_local_locator() {
        let root = temp_root("local-locator");
        let cache = temp_root("local-locator-cache");
        std::fs::create_dir_all(&root).expect("create local package");
        std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");

        let audit = audit_package_source_locator(
            root.display().to_string(),
            None,
            &cache,
            LocalSourceLimits::default(),
        )
        .expect("audit local locator");

        assert_eq!(audit.source_kind, "local-path");
        assert_eq!(audit.file_count, 1);

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn source_audit_command_reports_parse_errors_before_resolving() {
        assert_eq!(
            audit_package_source_locator(
                "./fixtures/packages/file-journal",
                Some("HEAD".to_owned()),
                temp_root("unused-cache"),
                LocalSourceLimits::default()
            ),
            Err(PackageSourceAuditCommandError::Parse(
                PackageSourceRequestParseError::LocalSourceCannotUseRevision {
                    locator: "./fixtures/packages/file-journal".to_owned(),
                    rev: "HEAD".to_owned(),
                }
            ))
        );
    }

    #[test]
    fn source_cache_policy_command_parses_and_records_local_locator() {
        let root = temp_root("source-cache-policy-local");
        let cache = temp_root("source-cache-policy-cache");
        std::fs::create_dir_all(&root).expect("create local package");
        std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");

        let record = resolve_source_cache_record_locator(
            root.display().to_string(),
            None,
            &cache,
            LocalSourceLimits::default(),
        )
        .expect("record local source cache policy");

        assert_eq!(record.source_kind, "local-path");
        assert_eq!(record.verdict.as_str(), "accepted");
        assert_eq!(record.file_count, Some(1));
        assert!(record.content_identity.as_ref().expect("identity").len() == 64);

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn source_cache_policy_command_reports_parse_errors_before_resolving() {
        assert_eq!(
            resolve_source_cache_record_locator(
                "./fixtures/packages/file-journal",
                Some("HEAD".to_owned()),
                temp_root("unused-cache-policy"),
                LocalSourceLimits::default()
            ),
            Err(SourceCachePolicyCommandError::Parse(
                PackageSourceRequestParseError::LocalSourceCannotUseRevision {
                    locator: "./fixtures/packages/file-journal".to_owned(),
                    rev: "HEAD".to_owned(),
                }
            ))
        );
    }

    #[test]
    fn git_source_audit_reports_commit_and_tree() {
        let repo = temp_root("git");
        let cache = temp_root("git-cache");
        std::fs::create_dir_all(&repo).expect("create git package");
        run_test_git(&repo, ["init", "--quiet"]);
        run_test_git(&repo, ["config", "user.email", "omega@example.invalid"]);
        run_test_git(&repo, ["config", "user.name", "Omega Tests"]);
        std::fs::write(repo.join("main.omg"), "machine Main::main() {}\n").expect("write source");
        run_test_git(&repo, ["add", "main.omg"]);
        run_test_git(&repo, ["commit", "--quiet", "-m", "initial"]);

        let audit = audit_package_source(
            PackageSourceRequest::Git {
                url: repo.display().to_string(),
                rev: Some("HEAD".to_owned()),
            },
            &cache,
            LocalSourceLimits::default(),
        )
        .expect("audit git source");

        assert_eq!(audit.source_kind, "git");
        assert_eq!(audit.file_count, 1);
        assert_eq!(audit.resolved_commit.as_ref().expect("commit").len(), 40);
        assert_eq!(audit.resolved_tree.as_ref().expect("tree").len(), 40);

        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn graph_audit_command_reads_lock_and_returns_text() {
        let root = temp_root("graph-audit");
        std::fs::create_dir_all(&root).expect("create audit temp");
        let lock_path = root.join("omega.lock");
        let root_manifest = manifest("graph-workbench");
        let mut child_manifest = manifest("file-journal");
        child_manifest
            .exported_service_reach
            .push("FilesystemHost".to_owned());
        let lock = graph_lock(&root_manifest, &child_manifest);
        lock.write_to_path(&lock_path).expect("write lock fixture");

        let report = audit_package_graph_from_lock(
            &lock_path,
            &[root_manifest.clone(), child_manifest.clone()],
        )
        .expect("audit command should succeed");
        let text = report.to_text();

        assert_eq!(report.lock_path, lock_path);
        assert!(text.contains("lock: "));
        assert!(text.contains("package graph audit"));
        assert!(text.contains("FilesystemHost via graph-workbench -> file-journal"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn graph_audit_command_rejects_missing_manifest() {
        let root = temp_root("graph-audit-missing");
        std::fs::create_dir_all(&root).expect("create audit temp");
        let lock_path = root.join("omega.lock");
        let root_manifest = manifest("graph-workbench");
        let child_manifest = manifest("file-journal");
        let lock = graph_lock(&root_manifest, &child_manifest);
        lock.write_to_path(&lock_path).expect("write lock fixture");

        assert_eq!(
            audit_package_graph_from_lock(&lock_path, &[root_manifest]),
            Err(PackageGraphAuditCommandError::Graph(
                PackageGraphAuditError::MissingManifest {
                    package: "file-journal".to_owned(),
                }
            ))
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn graph_audit_command_reads_lock_and_manifest_paths() {
        let root = temp_root("graph-audit-paths");
        std::fs::create_dir_all(&root).expect("create audit temp");
        let lock_path = root.join("omega.lock");
        let root_manifest_path = root.join("graph-workbench.package.json");
        let child_manifest_path = root.join("file-journal.package.json");
        let root_manifest = manifest("graph-workbench");
        let mut child_manifest = manifest("file-journal");
        child_manifest
            .exported_service_reach
            .push("FilesystemHost".to_owned());
        let lock = graph_lock(&root_manifest, &child_manifest);
        lock.write_to_path(&lock_path).expect("write lock fixture");
        root_manifest
            .write_to_path(&root_manifest_path)
            .expect("write root manifest");
        child_manifest
            .write_to_path(&child_manifest_path)
            .expect("write child manifest");

        let report = audit_package_graph_from_paths(
            &lock_path,
            &[root_manifest_path.clone(), child_manifest_path.clone()],
        )
        .expect("audit command should read manifests");

        assert_eq!(report.lock_path, lock_path);
        assert!(
            report
                .to_text()
                .contains("FilesystemHost via graph-workbench -> file-journal")
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn graph_audit_command_reports_manifest_path_parse_error() {
        let root = temp_root("graph-audit-bad-manifest");
        std::fs::create_dir_all(&root).expect("create audit temp");
        let lock_path = root.join("omega.lock");
        let manifest_path = root.join("bad.package.json");
        let root_manifest = manifest("graph-workbench");
        let lock = lock_from_manifests(std::slice::from_ref(&root_manifest));
        lock.write_to_path(&lock_path).expect("write lock fixture");
        std::fs::write(&manifest_path, "{").expect("write bad manifest");

        assert!(matches!(
            audit_package_graph_from_paths(&lock_path, std::slice::from_ref(&manifest_path)),
            Err(PackageGraphAuditFromPathsCommandError::Manifest { path, .. })
                if path == manifest_path
        ));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn lock_assembly_command_reads_manifests_writes_lock_and_audits() {
        let root = temp_root("lock-assembly-paths");
        std::fs::create_dir_all(&root).expect("create lock temp");
        let out_path = root.join("omega.lock");
        let root_manifest_path = root.join("graph-workbench.package.json");
        let child_manifest_path = root.join("file-journal.package.json");
        let mut root_manifest = manifest("graph-workbench");
        root_manifest
            .dependency_aliases
            .push(dependency("file_journal", "file-journal"));
        let mut child_manifest = manifest("file-journal");
        child_manifest
            .exported_service_reach
            .push("FilesystemHost".to_owned());
        root_manifest
            .write_to_path(&root_manifest_path)
            .expect("write root manifest");
        child_manifest
            .write_to_path(&child_manifest_path)
            .expect("write child manifest");

        let command = assemble_package_lock_from_paths(
            &package("graph-workbench"),
            &[root_manifest_path.clone(), child_manifest_path.clone()],
            &out_path,
        )
        .expect("assemble lock command should succeed");
        let written = PackageLock::read_from_path(&out_path).expect("read written lock");
        let text = command.to_text();

        assert_eq!(command.out_path, out_path);
        assert_eq!(written.root_package, package("graph-workbench"));
        assert_eq!(written.packages.len(), 2);
        assert!(text.contains("package lock assembled"));
        assert!(text.contains("lock fingerprint: "));
        assert!(text.contains("FilesystemHost via graph-workbench -> file-journal"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn lock_assembly_command_rejects_bad_manifest_without_writing_lock() {
        let root = temp_root("lock-assembly-bad-manifest");
        std::fs::create_dir_all(&root).expect("create lock temp");
        let manifest_path = root.join("bad.package.json");
        let out_path = root.join("omega.lock");
        std::fs::write(&manifest_path, "{").expect("write bad manifest");

        assert!(matches!(
            assemble_package_lock_from_paths(
                &package("graph-workbench"),
                std::slice::from_ref(&manifest_path),
                &out_path,
            ),
            Err(PackageLockAssemblyFromPathsCommandError::Manifest { path, .. })
                if path == manifest_path
        ));
        assert!(!out_path.exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn install_plan_command_reads_lock_and_returns_plan() {
        let root = temp_root("install-plan-command");
        std::fs::create_dir_all(&root).expect("create install temp");
        let lock_path = root.join("omega.lock");
        let current_root = manifest("graph-workbench");
        let current_lock = lock_from_manifests(std::slice::from_ref(&current_root));
        current_lock
            .write_to_path(&lock_path)
            .expect("write current lock");

        let mut candidate_root = current_root.clone();
        candidate_root
            .dependency_aliases
            .push(dependency("file_journal", "file-journal"));
        let child_manifest = manifest("file-journal");
        let candidate_manifests = vec![candidate_root, child_manifest];

        let command = plan_package_install_from_lock(
            &lock_path,
            &[current_root],
            &candidate_manifests,
            &alias("file_journal"),
            &package("file-journal"),
        )
        .expect("install command should plan");
        let text = command.to_text();

        assert_eq!(command.lock_path, lock_path);
        assert_eq!(command.plan.added_packages, vec!["file-journal".to_owned()]);
        assert!(text.contains("package install plan"));
        assert!(text.contains("added packages: file-journal"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn update_plan_command_reads_lock_and_receipt() {
        let root = temp_root("update-plan-command");
        std::fs::create_dir_all(&root).expect("create update temp");
        let lock_path = root.join("omega.lock");
        let receipt_path = root.join("capability-change.receipt.json");
        let mut root_manifest = manifest("graph-workbench");
        root_manifest
            .dependency_aliases
            .push(dependency("file_journal", "file-journal"));
        let current_child = manifest("file-journal");
        let current_manifests = vec![root_manifest.clone(), current_child.clone()];
        let current_lock = lock_from_manifests(&current_manifests);
        current_lock
            .write_to_path(&lock_path)
            .expect("write current lock");

        let mut candidate_child = manifest("file-journal");
        candidate_child.source.resolved = "commit:file-journal-new".to_owned();
        candidate_child
            .exported_service_reach
            .push("FilesystemHost".to_owned());
        let candidate_manifests = vec![root_manifest, candidate_child.clone()];
        let diff = diff_package_capability_manifests(&current_child, &candidate_child);
        let receipt = CapabilityChangeReceipt::from_diff(
            &diff,
            current_child.source.resolved,
            candidate_child.source.resolved,
            "reviewer@example.invalid",
            "audited filesystem reach",
        )
        .expect("receipt");
        receipt.write_to_path(&receipt_path).expect("write receipt");

        let command = plan_package_lock_update_from_lock(
            &lock_path,
            &current_manifests,
            &candidate_manifests,
            &package("file-journal"),
            Some(receipt_path.as_path()),
        )
        .expect("update command should plan");
        let text = command.to_text();

        assert_eq!(command.lock_path, lock_path);
        assert_eq!(command.receipt_path, Some(receipt_path.clone()));
        assert!(command.plan.is_admitted());
        assert!(text.contains("package lock update plan"));
        assert!(text.contains("admitted by capability-change receipt"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn capability_change_review_command_creates_exact_receipt() {
        let old = manifest("file-journal");
        let mut new = manifest("file-journal");
        new.source.resolved = "commit:file-journal-new".to_owned();
        new.exported_service_reach.push("FilesystemHost".to_owned());

        let command = create_capability_change_review(
            &old,
            &new,
            "reviewer@example.invalid",
            "audited filesystem reach",
        )
        .expect("capability change should create receipt");
        let text = command.to_text();

        assert!(command.receipt.accepts(&command.diff));
        assert_eq!(command.receipt.old_source_identity, "commit:file-journal");
        assert_eq!(
            command.receipt.new_source_identity,
            "commit:file-journal-new"
        );
        assert!(
            command
                .blocking_deltas
                .iter()
                .any(|delta| delta.section == "exported_service_reach")
        );
        assert!(text.contains("capability-change review receipt"));
        assert!(text.contains("exported_service_reach(high)"));
    }

    #[test]
    fn capability_change_review_command_rejects_source_only_or_noop() {
        let old = manifest("file-journal");
        let mut source_only = manifest("file-journal");
        source_only.source.resolved = "commit:file-journal-new".to_owned();

        assert_eq!(
            create_capability_change_review(
                &old,
                &source_only,
                "reviewer@example.invalid",
                "audited source-only update",
            ),
            Err(CapabilityChangeReviewCommandError::SourceOnlyChange {
                package: "file-journal".to_owned(),
            })
        );
        assert_eq!(
            create_capability_change_review(
                &old,
                &old,
                "reviewer@example.invalid",
                "audited no-op update",
            ),
            Err(CapabilityChangeReviewCommandError::NoManifestChange {
                package: "file-journal".to_owned(),
            })
        );
    }

    #[test]
    fn capability_change_review_command_rejects_package_mismatch() {
        assert_eq!(
            create_capability_change_review(
                &manifest("file-journal"),
                &manifest("arithmetic-kernels"),
                "reviewer@example.invalid",
                "audited package mismatch",
            ),
            Err(CapabilityChangeReviewCommandError::PackageMismatch {
                old_package: "file-journal".to_owned(),
                new_package: "arithmetic-kernels".to_owned(),
            })
        );
    }
}
