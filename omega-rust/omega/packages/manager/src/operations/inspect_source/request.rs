use package_source::{GitSourceRequest, GitSourceRequestError, SourceResolveError};
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
pub enum PackageSourceInspectionError {
    Parse(PackageSourceRequestParseError),
    Resolve(SourceResolveError),
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
