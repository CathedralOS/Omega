//! Git source behavior, organized by the resolver invariant under test.

use super::cache::identity::*;
use super::executable::executor::test_system_git_executor;
use super::objects::{batch::*, tree::*, *};
use super::request::*;
use super::resolution::*;
use super::snapshot::{materialization::*, metadata::*, permissions::*};
use super::workspace::*;
use crate::error::SourceResolveError;
use crate::identity::*;
use crate::limits::*;
use crate::local::capture::{raw_os_bytes, resolve_materialized_source};
use crate::storage::SourceResolverStorage;
use crate::test_support::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod request;
mod reuse;
mod snapshot;
mod validation;
mod workspace;
