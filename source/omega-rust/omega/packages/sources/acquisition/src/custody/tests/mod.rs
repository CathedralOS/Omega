//! Cache custody behavior, organized by the invariant under test.

#[cfg(unix)]
use crate::custody::platform::*;
use crate::custody::{lock::*, publication::*, tree::*};
use crate::error::SourceResolveError;
#[cfg(unix)]
use crate::error::cache_invalid;
use crate::git::cache::identity::*;
#[cfg(unix)]
use crate::git::cache::invalidation::*;
#[cfg(unix)]
use crate::git::commands::reconciliation::*;
#[cfg(unix)]
use crate::git::executable::executor::*;
use crate::git::request::GitExecutionTransport;
use crate::limits::*;
#[cfg(unix)]
use crate::local::operations::*;
use crate::snapshot::{construction::*, permissions::*, publication::*};
#[cfg(unix)]
use crate::storage::SourceResolverStorage;
use crate::test_support::*;
use crate::tree::filesystem::*;
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
