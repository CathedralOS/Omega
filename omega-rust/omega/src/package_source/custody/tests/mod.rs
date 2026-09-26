//! Cache custody behavior, organized by the invariant under test.

#[cfg(unix)]
use crate::package_source::custody::platform::*;
use crate::package_source::custody::{lock::*, publication::*, tree::*};
use crate::package_source::error::SourceResolveError;
#[cfg(unix)]
use crate::package_source::error::cache_invalid;
use crate::package_source::git::cache::identity::*;
#[cfg(unix)]
use crate::package_source::git::cache::invalidation::*;
#[cfg(unix)]
use crate::package_source::git::executable::executor::*;
#[cfg(unix)]
use crate::package_source::git::git_command::reconciliation::*;
use crate::package_source::git::request::GitExecutionTransport;
use crate::package_source::limits::*;
#[cfg(unix)]
use crate::package_source::local::operations::*;
use crate::package_source::snapshot::{construction::*, permissions::*, publication::*};
#[cfg(unix)]
use crate::package_source::storage::SourceResolverStorage;
use crate::package_source::test_support::*;
use crate::package_source::tree::filesystem::*;
use std::fs::OpenOptions;

mod identity;
mod limits;
mod locks;
#[cfg(target_os = "macos")]
mod macos_acl;
mod publication;
mod repository_integrity;
mod snapshots;
#[cfg(unix)]
mod traversal;
