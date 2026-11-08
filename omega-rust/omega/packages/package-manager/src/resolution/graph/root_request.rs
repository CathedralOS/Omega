//! Exact request that selected a dependency graph root.

use crate::resolution::source::GitPackageSourceRequest;
use package_source::ExternalSourceContext;
use std::path::PathBuf;

/// Dependency requests belong to a requester package. The graph root has no
/// requester, so its exact source request is retained separately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PackageRootSourceRequest {
    Git(GitPackageSourceRequest),
    ExternalLocal {
        requested_root: PathBuf,
        source_context: ExternalSourceContext,
    },
}
