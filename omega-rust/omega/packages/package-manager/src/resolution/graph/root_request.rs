// We record the exact source request that selected a dependency graph root.
// Dependency requests belong to a requester package; because the graph root has
// no requester, its exact source request is retained separately.

use crate::resolution::source::GitPackageSourceRequest;
use package_source::ExternalSourceContext;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PackageRootSourceRequest {
    Git(GitPackageSourceRequest),
    ExternalLocal {
        requested_root: PathBuf,
        source_context: ExternalSourceContext,
    },
}
