use crate::source::{
    GitSourceSpec, LocalSourceLimits, SourceResolveError, resolve_git_source, resolve_local_source,
};
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
}
