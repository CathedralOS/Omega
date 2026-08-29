//! Hostile local and Git source acquisition under immutable snapshot custody.
//!
//! The public entrance stays deliberately small. Local capture and publication
//! live under [`local`]; Git request validation, object authentication,
//! materialization, cache custody, and process execution live under [`git`].

use crate::resolution::identity::{
    GitObjectIdAlgorithm, GitRequestedNetworkEndpoint, GitTransport, IdentityError,
    SourceContentDigest, SourceLineage,
};
use crate::storage::record_file::{RecordFileLimits, RecordFileRoot};
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
#[cfg(unix)]
use cap_std::fs::{
    DirBuilderExt as CapabilityDirBuilderExt, OpenOptionsExt as CapabilityOpenOptionsExt,
    PermissionsExt as CapabilityPermissionsExt,
};
use cap_std::{
    ambient_authority,
    fs::{
        Dir as CapabilityDirectory, DirBuilder as CapabilityDirBuilder,
        Metadata as CapabilityMetadata, OpenOptions as CapabilityOpenOptions,
    },
};
use omega_resolver_execution::{
    RESOLVER_CONNECT_BROKER_ENVIRONMENT, RESOLVER_CONNECT_HELPER_BASENAME,
    RESOLVER_CONNECT_TARGET_ENVIRONMENT, ResolverExecutionBackend, ResolverExecutionChild,
    ResolverExecutionEndpointObservation, ResolverExecutionEndpointOutcome,
    ResolverExecutionEndpointRoute, ResolverExecutionNetworkTransport, ResolverExecutionPhase,
    ResolverExecutionPolicyObservation, ResolverExecutionRequestedEndpoint,
    ResolverExecutionTransferBudget,
};
use sha1_checked::Sha1 as CheckedSha1;
use sha2::{Digest, Sha256};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::File;
#[cfg(test)]
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant, SystemTime};

mod custody;
mod error;
mod git;
mod limits;
mod local;
mod observations;

use custody::*;
use git::execution::{GitExecutor, format_sha256};
use git::*;
use limits::*;
use local::*;
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
    GitCapturedOutputObservation, GitCommandExecutionObservation, GitExecutableIdentity,
    GitNetworkTransferObservation, GitSourceResolutionObservation, GitTransportExecutableIdentity,
    ResolvedGitSource,
};

#[cfg(test)]
mod tests;
