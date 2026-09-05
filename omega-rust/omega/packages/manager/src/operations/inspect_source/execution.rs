//! Source acquisition and report assembly for source inspection.

use super::report::PackageSourceInspection;
#[cfg(test)]
use super::request::PackageSourceRequestParseError;
use super::request::{PackageSourceInspectionError, PackageSourceRequest, SourceAdapter};
use package_source::git::resolution::resolve_git_source_in_lane;
#[cfg(test)]
use package_source::resolve_git_source_with_storage;
use package_source::storage::RetainedStorageLane;
use package_source::{
    LocalSourceLimits, SourceResolveError, SourceResolverStorage, resolve_local_source,
};
#[cfg(test)]
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

#[cfg(test)]
pub(crate) fn inspect_package_source_in_cache(
    request: PackageSourceRequest,
    cache_dir: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<PackageSourceInspection, SourceResolveError> {
    match request {
        PackageSourceRequest::LocalPath(path) => {
            let resolved = resolve_local_source(&path, limits)?;
            Ok(PackageSourceInspection {
                source_kind: "local-path".to_owned(),
                locator: path.display().to_string(),
                transport_profile: None,
                requested_rev: None,
                resolved_commit: None,
                resolved_tree: None,
                content_identity: resolved.content_identity,
                file_count: resolved.file_count,
                byte_count: resolved.byte_count,
            })
        }
        PackageSourceRequest::Git(request) => {
            let storage = SourceResolverStorage::for_hardened_base(cache_dir)?;
            let resolved = resolve_git_source_with_storage(&request, &storage, limits)?;
            Ok(PackageSourceInspection {
                source_kind: "git".to_owned(),
                locator: request.locator_identity().to_owned(),
                transport_profile: Some(resolved.transport_profile().as_str().to_owned()),
                requested_rev: Some(resolved.requested_revision().to_owned()),
                resolved_commit: Some(resolved.commit().to_owned()),
                resolved_tree: Some(resolved.tree().to_owned()),
                content_identity: resolved.local().content_identity.clone(),
                file_count: resolved.local().file_count,
                byte_count: resolved.local().byte_count,
            })
        }
    }
}

fn inspect_package_source_in_lane(
    request: PackageSourceRequest,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<PackageSourceInspection, SourceResolveError> {
    match request {
        PackageSourceRequest::LocalPath(path) => {
            let resolved = resolve_local_source(&path, limits)?;
            Ok(PackageSourceInspection {
                source_kind: "local-path".to_owned(),
                locator: path.display().to_string(),
                transport_profile: None,
                requested_rev: None,
                resolved_commit: None,
                resolved_tree: None,
                content_identity: resolved.content_identity,
                file_count: resolved.file_count,
                byte_count: resolved.byte_count,
            })
        }
        PackageSourceRequest::Git(request) => {
            let resolved = resolve_git_source_in_lane(&request, lane, limits)?;
            Ok(PackageSourceInspection {
                source_kind: "git".to_owned(),
                locator: request.locator_identity().to_owned(),
                transport_profile: Some(resolved.transport_profile().as_str().to_owned()),
                requested_rev: Some(resolved.requested_revision().to_owned()),
                resolved_commit: Some(resolved.commit().to_owned()),
                resolved_tree: Some(resolved.tree().to_owned()),
                content_identity: resolved.local().content_identity.clone(),
                file_count: resolved.local().file_count,
                byte_count: resolved.local().byte_count,
            })
        }
    }
}

/// Inspect one source using manager-owned private resolver storage.
pub fn inspect_package_source(
    request: PackageSourceRequest,
    storage: &SourceResolverStorage,
    limits: LocalSourceLimits,
) -> Result<PackageSourceInspection, SourceResolveError> {
    storage.verify_path_identity()?;
    let result = inspect_package_source_in_lane(request, storage.git_sources(), limits);
    storage.verify_path_identity()?;
    result
}

#[cfg(test)]
pub(crate) fn inspect_package_source_locator_in_cache(
    adapter: SourceAdapter,
    locator: impl Into<String>,
    rev: Option<String>,
    cache_dir: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<PackageSourceInspection, PackageSourceInspectionError> {
    let request = PackageSourceRequest::parse(adapter, locator, rev)
        .map_err(PackageSourceInspectionError::Parse)?;
    inspect_package_source_in_cache(request, cache_dir, limits)
        .map_err(PackageSourceInspectionError::Resolve)
}

/// Parse and inspect one source using manager-owned private resolver storage.
pub fn inspect_package_source_locator(
    adapter: SourceAdapter,
    locator: impl Into<String>,
    rev: Option<String>,
    storage: &SourceResolverStorage,
    limits: LocalSourceLimits,
) -> Result<PackageSourceInspection, PackageSourceInspectionError> {
    let request = PackageSourceRequest::parse(adapter, locator, rev)
        .map_err(PackageSourceInspectionError::Parse)?;
    inspect_package_source(request, storage, limits).map_err(PackageSourceInspectionError::Resolve)
}

#[cfg(test)]
mod tests {
    use super::*;
    use package_source::GitSourceRequest;
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
            inspect_package_source_locator_in_cache(
                SourceAdapter::Git,
                "http://github.com/CathedralOS/tool.git",
                None,
                ".",
                LocalSourceLimits::default(),
            ),
            Err(PackageSourceInspectionError::Parse(
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
        let storage = SourceResolverStorage::for_hardened_base(&cache_base)
            .expect("create private resolver storage");
        storage
            .verify_path_identity()
            .expect("storage identity before source audit");

        let low_level = inspect_package_source_in_cache(
            PackageSourceRequest::LocalPath(root.clone()),
            storage.git_sources().path(),
            LocalSourceLimits::default(),
        )
        .expect("audit local source in explicit cache");
        let direct = inspect_package_source(
            PackageSourceRequest::LocalPath(root.clone()),
            &storage,
            LocalSourceLimits::default(),
        )
        .expect("audit local source through managed storage");
        let wrapped = inspect_package_source_locator(
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
        assert!(direct.to_text().contains("package source inspection"));
        let report = direct.to_text();
        assert!(!report.contains("broker transfer ceiling:"));
        assert!(!report.contains("broker uploaded bytes:"));
        assert!(!report.contains("broker downloaded bytes:"));
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
        let storage = SourceResolverStorage::for_hardened_base(&cache_base)
            .expect("create private resolver storage");
        storage
            .verify_path_identity()
            .expect("storage identity before Git source inspection");

        let inspection = inspect_package_source(
            PackageSourceRequest::Git(
                GitSourceRequest::for_local_test_repository(&repository, Some("HEAD".to_owned()))
                    .expect("local Git fixture request"),
            ),
            &storage,
            LocalSourceLimits::default(),
        )
        .expect("inspect git source");
        storage
            .verify_path_identity()
            .expect("storage identity after Git source inspection");

        assert_eq!(inspection.source_kind, "git");
        assert_eq!(inspection.file_count, 1);
        assert_eq!(
            inspection.resolved_commit.as_ref().expect("commit").len(),
            40
        );
        assert_eq!(inspection.resolved_tree.as_ref().expect("tree").len(), 40);
        let report = inspection.to_text();
        assert!(!report.contains("broker transfer ceiling:"));
        assert!(!report.contains("broker uploaded bytes:"));
        assert!(!report.contains("broker downloaded bytes:"));

        drop(storage);
        let _ = std::fs::remove_dir_all(&repository);
        let _ = std::fs::remove_dir_all(&cache_base);
    }
}
