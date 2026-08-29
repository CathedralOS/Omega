//! Read-only source inspection used by the command-facing diagnostic surface.

use crate::resolution::source::{RetainedStorageLane, resolve_git_source_in_lane};
#[cfg(test)]
use crate::source::resolve_git_source_with_storage;
use crate::source::{
    GitSourceRequest, GitSourceRequestError, LocalSourceLimits, SourceResolveError,
    SourceResolverStorage, resolve_local_source,
};
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceAdapter {
    Local,
    Git,
}

impl SourceAdapter {
    pub fn parse(value: &str) -> Result<Self, PackageSourceRequestParseError> {
        match value {
            "local" => Ok(Self::Local),
            "git" => Ok(Self::Git),
            _ => Err(PackageSourceRequestParseError::UnsupportedSourceAdapter {
                adapter: value.to_owned(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSourceRequest {
    LocalPath(PathBuf),
    Git(GitSourceRequest),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSourceRequestParseError {
    UnsupportedSourceAdapter { adapter: String },
    EmptySourceLocator,
    LocalSourceCannotUseRevision { locator: String, rev: String },
    UnsupportedFileUrl { locator: String },
    InvalidGitRequest(GitSourceRequestError),
}

impl PackageSourceRequest {
    pub fn parse(
        adapter: SourceAdapter,
        locator: impl Into<String>,
        rev: Option<String>,
    ) -> Result<Self, PackageSourceRequestParseError> {
        let locator = locator.into();
        if locator.trim().is_empty() {
            return Err(PackageSourceRequestParseError::EmptySourceLocator);
        }
        match adapter {
            SourceAdapter::Local => {
                let path = file_url_path(&locator)?.unwrap_or_else(|| PathBuf::from(&locator));
                reject_local_rev(&locator, &rev)?;
                Ok(Self::LocalPath(path))
            }
            SourceAdapter::Git => GitSourceRequest::new(locator, rev)
                .map(Self::Git)
                .map_err(PackageSourceRequestParseError::InvalidGitRequest),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSourceAudit {
    pub source_kind: String,
    pub locator: String,
    pub transport_profile: Option<String>,
    pub requested_rev: Option<String>,
    pub resolved_commit: Option<String>,
    pub resolved_tree: Option<String>,
    pub network_transfer_ceiling: Option<u64>,
    pub network_uploaded_bytes: Option<u64>,
    pub network_downloaded_bytes: Option<u64>,
    pub content_identity: String,
    pub file_count: usize,
    pub byte_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSourceAuditCommandError {
    Parse(PackageSourceRequestParseError),
    Resolve(SourceResolveError),
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
        if let Some(transport_profile) = &self.transport_profile {
            report.push_str("transport profile: ");
            report.push_str(transport_profile);
            report.push('\n');
        }
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
        if let Some(ceiling) = self.network_transfer_ceiling {
            report.push_str("broker transfer ceiling: ");
            report.push_str(&ceiling.to_string());
            report.push('\n');
        }
        if let Some(uploaded) = self.network_uploaded_bytes {
            report.push_str("broker uploaded bytes: ");
            report.push_str(&uploaded.to_string());
            report.push('\n');
        }
        if let Some(downloaded) = self.network_downloaded_bytes {
            report.push_str("broker downloaded bytes: ");
            report.push_str(&downloaded.to_string());
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

#[cfg(test)]
pub(crate) fn audit_package_source_in_cache(
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
                transport_profile: None,
                requested_rev: None,
                resolved_commit: None,
                resolved_tree: None,
                network_transfer_ceiling: None,
                network_uploaded_bytes: None,
                network_downloaded_bytes: None,
                content_identity: resolved.content_identity,
                file_count: resolved.file_count,
                byte_count: resolved.byte_count,
            })
        }
        PackageSourceRequest::Git(request) => {
            let storage = SourceResolverStorage::for_hardened_base(cache_dir)?;
            let resolved = resolve_git_source_with_storage(&request, &storage, limits)?;
            let network_transfer = resolved.network_transfer_observation();
            Ok(PackageSourceAudit {
                source_kind: "git".to_owned(),
                locator: request.locator_identity().to_owned(),
                transport_profile: Some(resolved.transport_profile().as_str().to_owned()),
                requested_rev: Some(resolved.requested_revision().to_owned()),
                resolved_commit: Some(resolved.commit().to_owned()),
                resolved_tree: Some(resolved.tree().to_owned()),
                network_transfer_ceiling: Some(network_transfer.ceiling()),
                network_uploaded_bytes: Some(network_transfer.uploaded()),
                network_downloaded_bytes: Some(network_transfer.downloaded()),
                content_identity: resolved.local().content_identity.clone(),
                file_count: resolved.local().file_count,
                byte_count: resolved.local().byte_count,
            })
        }
    }
}

fn audit_package_source_in_lane(
    request: PackageSourceRequest,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<PackageSourceAudit, SourceResolveError> {
    match request {
        PackageSourceRequest::LocalPath(path) => {
            let resolved = resolve_local_source(&path, limits)?;
            Ok(PackageSourceAudit {
                source_kind: "local-path".to_owned(),
                locator: path.display().to_string(),
                transport_profile: None,
                requested_rev: None,
                resolved_commit: None,
                resolved_tree: None,
                network_transfer_ceiling: None,
                network_uploaded_bytes: None,
                network_downloaded_bytes: None,
                content_identity: resolved.content_identity,
                file_count: resolved.file_count,
                byte_count: resolved.byte_count,
            })
        }
        PackageSourceRequest::Git(request) => {
            let resolved = resolve_git_source_in_lane(&request, lane, limits)?;
            let network_transfer = resolved.network_transfer_observation();
            Ok(PackageSourceAudit {
                source_kind: "git".to_owned(),
                locator: request.locator_identity().to_owned(),
                transport_profile: Some(resolved.transport_profile().as_str().to_owned()),
                requested_rev: Some(resolved.requested_revision().to_owned()),
                resolved_commit: Some(resolved.commit().to_owned()),
                resolved_tree: Some(resolved.tree().to_owned()),
                network_transfer_ceiling: Some(network_transfer.ceiling()),
                network_uploaded_bytes: Some(network_transfer.uploaded()),
                network_downloaded_bytes: Some(network_transfer.downloaded()),
                content_identity: resolved.local().content_identity.clone(),
                file_count: resolved.local().file_count,
                byte_count: resolved.local().byte_count,
            })
        }
    }
}

/// Audit one source using manager-owned private resolver storage.
pub fn audit_package_source(
    request: PackageSourceRequest,
    storage: &SourceResolverStorage,
    limits: LocalSourceLimits,
) -> Result<PackageSourceAudit, SourceResolveError> {
    storage.verify_path_identity()?;
    let result = audit_package_source_in_lane(request, storage.git_sources(), limits);
    storage.verify_path_identity()?;
    result
}

#[cfg(test)]
pub(crate) fn audit_package_source_locator_in_cache(
    adapter: SourceAdapter,
    locator: impl Into<String>,
    rev: Option<String>,
    cache_dir: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<PackageSourceAudit, PackageSourceAuditCommandError> {
    let request = PackageSourceRequest::parse(adapter, locator, rev)
        .map_err(PackageSourceAuditCommandError::Parse)?;
    audit_package_source_in_cache(request, cache_dir, limits)
        .map_err(PackageSourceAuditCommandError::Resolve)
}

/// Parse and audit one source using manager-owned private resolver storage.
pub fn audit_package_source_locator(
    adapter: SourceAdapter,
    locator: impl Into<String>,
    rev: Option<String>,
    storage: &SourceResolverStorage,
    limits: LocalSourceLimits,
) -> Result<PackageSourceAudit, PackageSourceAuditCommandError> {
    let request = PackageSourceRequest::parse(adapter, locator, rev)
        .map_err(PackageSourceAuditCommandError::Parse)?;
    audit_package_source(request, storage, limits).map_err(PackageSourceAuditCommandError::Resolve)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "omega-package-source-command-{name}-{}-{stamp}",
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

    #[test]
    fn source_adapter_is_explicit() {
        assert_eq!(SourceAdapter::parse("local"), Ok(SourceAdapter::Local));
        assert_eq!(SourceAdapter::parse("git"), Ok(SourceAdapter::Git));
        assert!(SourceAdapter::parse("https").is_err());
    }

    #[test]
    fn local_source_rejects_revisions_and_relative_file_urls() {
        assert!(matches!(
            PackageSourceRequest::parse(SourceAdapter::Local, "source", Some("main".to_owned())),
            Err(PackageSourceRequestParseError::LocalSourceCannotUseRevision { .. })
        ));
        assert!(matches!(
            PackageSourceRequest::parse(SourceAdapter::Local, "file://relative", None),
            Err(PackageSourceRequestParseError::UnsupportedFileUrl { .. })
        ));
    }

    #[test]
    fn local_source_accepts_paths_and_absolute_file_urls() {
        assert_eq!(
            PackageSourceRequest::parse(SourceAdapter::Local, "./source", None),
            Ok(PackageSourceRequest::LocalPath(PathBuf::from("./source")))
        );
        assert_eq!(
            PackageSourceRequest::parse(SourceAdapter::Local, "file:///tmp/source", None),
            Ok(PackageSourceRequest::LocalPath(PathBuf::from(
                "/tmp/source"
            )))
        );
        assert_eq!(
            PackageSourceRequest::parse(SourceAdapter::Local, "", None),
            Err(PackageSourceRequestParseError::EmptySourceLocator)
        );
    }

    #[test]
    fn git_source_routes_share_validated_sanitized_requests() {
        let request = PackageSourceRequest::parse(
            SourceAdapter::Git,
            "git@github.com:CathedralOS/Arithmetic-Kernels.git",
            Some("refs/heads/main".to_owned()),
        )
        .expect("valid Git request");
        let PackageSourceRequest::Git(request) = request else {
            panic!("Git adapter returned a local request");
        };
        assert_eq!(
            request.locator_identity(),
            "https://github.com/cathedralos/arithmetic-kernels.git"
        );
        assert_eq!(request.requested_revision(), "refs/heads/main");
        assert_eq!(request.transport_profile().as_str(), "ssh");

        for locator in [
            "http://github.com/CathedralOS/tool.git",
            "https://token@github.com/CathedralOS/tool.git",
            "file:///tmp/tool.git",
            " /tmp/tool.git ",
        ] {
            assert!(matches!(
                PackageSourceRequest::parse(SourceAdapter::Git, locator, None),
                Err(PackageSourceRequestParseError::InvalidGitRequest(_))
            ));
        }

        assert!(matches!(
            audit_package_source_locator_in_cache(
                SourceAdapter::Git,
                "http://github.com/CathedralOS/tool.git",
                None,
                ".",
                LocalSourceLimits::default(),
            ),
            Err(PackageSourceAuditCommandError::Parse(
                PackageSourceRequestParseError::InvalidGitRequest(_)
            ))
        ));
    }

    #[test]
    fn local_source_audit_and_locator_wrapper_report_resolved_identity() {
        let root = temp_root("local-audit");
        let cache_base = temp_root("local-audit-cache");
        std::fs::create_dir_all(&root).expect("create local package");
        std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");
        let storage = SourceResolverStorage::create_beneath(&cache_base)
            .expect("create private resolver storage");
        storage
            .verify_path_identity()
            .expect("storage identity before source audit");

        let low_level = audit_package_source_in_cache(
            PackageSourceRequest::LocalPath(root.clone()),
            storage.git_sources().path(),
            LocalSourceLimits::default(),
        )
        .expect("audit local source in explicit cache");
        let direct = audit_package_source(
            PackageSourceRequest::LocalPath(root.clone()),
            &storage,
            LocalSourceLimits::default(),
        )
        .expect("audit local source through managed storage");
        let wrapped = audit_package_source_locator(
            SourceAdapter::Local,
            root.display().to_string(),
            None,
            &storage,
            LocalSourceLimits::default(),
        )
        .expect("audit local locator");
        storage
            .verify_path_identity()
            .expect("storage identity after source audit");

        assert_eq!(direct.source_kind, "local-path");
        assert_eq!(direct.file_count, 1);
        assert_eq!(direct.content_identity.len(), 64);
        assert!(direct.to_text().contains("package source audit"));
        assert!(direct.network_transfer_ceiling.is_none());
        assert_eq!(low_level.content_identity, direct.content_identity);
        assert_eq!(wrapped.content_identity, direct.content_identity);

        drop(storage);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&cache_base);
    }

    #[test]
    fn git_source_audit_reports_commit_and_tree() {
        let repository = temp_root("git-audit");
        let cache_base = temp_root("git-audit-cache");
        std::fs::create_dir_all(&repository).expect("create git package");
        run_test_git(&repository, ["init", "--quiet"]);
        run_test_git(
            &repository,
            ["config", "user.email", "omega@example.invalid"],
        );
        run_test_git(&repository, ["config", "user.name", "Omega Tests"]);
        std::fs::write(repository.join("main.omg"), "machine Main::main() {}\n")
            .expect("write source");
        run_test_git(&repository, ["add", "main.omg"]);
        run_test_git(&repository, ["commit", "--quiet", "-m", "initial"]);
        let storage = SourceResolverStorage::create_beneath(&cache_base)
            .expect("create private resolver storage");
        storage
            .verify_path_identity()
            .expect("storage identity before Git source audit");

        let audit = audit_package_source(
            PackageSourceRequest::Git(
                GitSourceRequest::for_local_test_repository(&repository, Some("HEAD".to_owned()))
                    .expect("local Git fixture request"),
            ),
            &storage,
            LocalSourceLimits::default(),
        )
        .expect("audit git source");
        storage
            .verify_path_identity()
            .expect("storage identity after Git source audit");

        assert_eq!(audit.source_kind, "git");
        assert_eq!(audit.file_count, 1);
        assert_eq!(audit.resolved_commit.as_ref().expect("commit").len(), 40);
        assert_eq!(audit.resolved_tree.as_ref().expect("tree").len(), 40);
        assert!(audit.network_transfer_ceiling.is_some());
        assert_eq!(audit.network_uploaded_bytes, Some(0));
        assert_eq!(audit.network_downloaded_bytes, Some(0));
        assert!(audit.to_text().contains("broker transfer ceiling: "));

        drop(storage);
        let _ = std::fs::remove_dir_all(&repository);
        let _ = std::fs::remove_dir_all(&cache_base);
    }
}
