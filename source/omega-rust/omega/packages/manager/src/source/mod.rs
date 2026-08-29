//! Package identity and hostile source acquisition.
//!
//! [`identity`] names immutable package sources, [`local`] and [`git`] capture
//! hostile source under shared [`custody`], [`package`] binds retained source to
//! one declared package, and [`audit`] exposes the read-only command boundary.
//! Complete dependency-graph construction lives in the sibling [`crate::graph`]
//! responsibility.

use cap_fs_ext::FollowSymlinks;
#[cfg(test)]
use cap_std::ambient_authority;
use cap_std::fs::{
    Dir as CapabilityDirectory, Metadata as CapabilityMetadata,
    OpenOptions as CapabilityOpenOptions,
};
#[cfg(unix)]
use cap_std::fs::{
    OpenOptionsExt as CapabilityOpenOptionsExt, PermissionsExt as CapabilityPermissionsExt,
};
use omega_resolver_execution::ResolverExecutionPhase;
#[cfg(test)]
use omega_resolver_execution::ResolverExecutionRequestedEndpoint;
#[cfg(test)]
use omega_resolver_execution::{
    RESOLVER_CONNECT_BROKER_ENVIRONMENT, RESOLVER_CONNECT_HELPER_BASENAME,
    RESOLVER_CONNECT_TARGET_ENVIRONMENT,
};
use sha1_checked::Sha1 as CheckedSha1;
#[cfg(test)]
use sha2::Digest;
#[cfg(test)]
use std::collections::BTreeSet;
#[cfg(test)]
use std::ffi::OsStr;
#[cfg(test)]
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
#[cfg(test)]
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::process::{Command, Stdio};
#[cfg(test)]
use std::sync::Arc;

pub(crate) mod audit;
mod custody;
mod error;
mod git;
pub(crate) mod identity;
mod limits;
mod local;
mod observations;
pub(crate) mod package;
mod storage;

use custody::*;
use git::format_sha256;
use git::*;
use limits::*;
use local::*;
#[cfg(test)]
use observations::*;

pub use audit::{
    PackageSourceAudit, PackageSourceAuditCommandError, PackageSourceRequest,
    PackageSourceRequestParseError, SourceAdapter, audit_package_source,
    audit_package_source_locator,
};
pub use error::SourceResolveError;
pub use git::request::{GitSourceRequest, GitSourceRequestError, GitTransportProfile};
pub(crate) use git::resolve::resolve_git_source_in_lane;
pub use git::resolve::resolve_git_source_with_storage;
pub use identity::{
    AliasName, ExternalLocalLineage, ExternalSourceContext, GenericGitLineage, GitCommitId,
    GitHubRepositoryLineage, GitLabRepositoryLineage, GitObjectIdAlgorithm, GitTransport,
    GitTreeId, IdentityError, ImmutableSourceResolution, PackageKey, PackageName,
    SourceContentDigest, SourceLineage, WorkspaceLineageIdentity, WorkspaceMemberLineage,
    WorkspaceMemberPath,
};
pub use limits::LocalSourceLimits;
#[cfg(test)]
pub(crate) use local::resolve_local_source_snapshot_at_path;
pub(crate) use local::resolve_local_source_snapshot_in_lane;
pub use local::{
    LocalSourceResolutionObservation, ResolvedLocalSnapshot, ResolvedLocalSource,
    resolve_local_source, resolve_local_source_snapshot_with_storage,
};
pub(crate) use local::{
    VerifiedPackageSourceEntry, VerifiedPackageSourceEntryKind,
    capture_verified_package_source_snapshot, verify_package_source_snapshot,
};
pub use observations::{
    GitExecutableIdentity, GitNetworkTransferObservation, GitSourceResolutionObservation,
    GitTransportExecutableIdentity, ResolvedGitSource,
};
pub use package::{
    ResolvePackageSourceError, ResolvedPackageSource,
    resolve_external_local_package_source_with_storage,
    resolve_external_local_project_source_with_storage, resolve_git_package_source_with_storage,
    resolve_workspace_member_package_source_with_storage,
};
pub(crate) use storage::RetainedStorageLane;
pub use storage::SourceResolverStorage;

#[cfg(test)]
pub(crate) use package::{
    resolve_external_local_package_source, resolve_workspace_member_package_source,
};

#[cfg(test)]
mod tests;
