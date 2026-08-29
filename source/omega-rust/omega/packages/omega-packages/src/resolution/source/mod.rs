//! Hostile local and Git source acquisition under immutable snapshot custody.
//!
//! The public entrance stays deliberately small. Local capture and publication
//! live under [`local`]; Git request validation, object authentication,
//! materialization, cache custody, and process execution live under [`git`].

use crate::resolution::identity::GitObjectIdAlgorithm;
#[cfg(test)]
use crate::resolution::identity::SourceContentDigest;
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

mod custody;
mod error;
mod git;
mod limits;
mod local;
mod observations;
mod storage;

use custody::*;
use git::execution::format_sha256;
use git::*;
use limits::*;
use local::*;
#[cfg(test)]
use observations::*;

pub use error::SourceResolveError;
pub use git::request::{GitSourceRequest, GitSourceRequestError, GitTransportProfile};
pub use git::resolve::resolve_git_source;
pub use limits::LocalSourceLimits;
pub use local::{
    ResolvedLocalSnapshot, ResolvedLocalSource, resolve_local_source, resolve_local_source_snapshot,
};
pub(crate) use local::{
    VerifiedPackageSourceEntry, VerifiedPackageSourceEntryKind,
    capture_verified_package_source_snapshot, verify_package_source_snapshot,
};
pub use observations::{
    GitExecutableIdentity, GitNetworkTransferObservation, GitSourceResolutionObservation,
    GitTransportExecutableIdentity, ResolvedGitSource,
};
pub use storage::SourceResolverStorage;

#[cfg(test)]
mod tests;
