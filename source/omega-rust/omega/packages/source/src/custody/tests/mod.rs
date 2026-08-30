//! Cache custody behavior, organized by the invariant under test.

use crate::custody::{lock::*, platform::*, publication::*, tree::*};
use crate::error::{SourceResolveError, cache_invalid};
use crate::git::cache::{identity::*, invalidation::*};
use crate::git::commands::reconciliation::*;
use crate::git::executable::executor::*;
use crate::git::request::GitExecutionTransport;
use crate::limits::*;
use crate::local::operations::*;
use crate::snapshot::{construction::*, permissions::*, publication::*};
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
