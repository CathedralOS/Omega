//! Git source behavior, organized by the resolver invariant under test.

use super::cache::identity::*;
use super::executable::executor::test_system_git_executor;
use super::objects::{batch::*, tree::*, *};
use super::request::*;
use super::resolution::*;
use super::snapshot::*;
#[cfg(unix)]
use super::snapshot_metadata::*;
use super::workspace::*;
use crate::package_source::error::SourceResolveError;
use crate::package_source::identity::*;
use crate::package_source::limits::*;
use crate::package_source::snapshot::permissions::*;
use crate::package_source::storage::SourceResolverStorage;
use crate::package_source::test_support::*;
#[cfg(unix)]
use crate::package_source::tree::capture::resolve_materialized_source;
#[cfg(unix)]
use crate::package_source::tree::filesystem::raw_os_bytes;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod exact_revision;
mod exact_workspace;
mod request;
mod reuse;
mod root_pin;
mod snapshot;
mod validation;
mod workspace;
