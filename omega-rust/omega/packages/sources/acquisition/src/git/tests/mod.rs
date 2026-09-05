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
use crate::error::SourceResolveError;
use crate::identity::*;
use crate::limits::*;
use crate::snapshot::permissions::*;
use crate::storage::SourceResolverStorage;
use crate::test_support::*;
#[cfg(unix)]
use crate::tree::capture::resolve_materialized_source;
#[cfg(unix)]
use crate::tree::filesystem::raw_os_bytes;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod request;
mod reuse;
mod root_pin;
mod snapshot;
mod validation;
mod workspace;
