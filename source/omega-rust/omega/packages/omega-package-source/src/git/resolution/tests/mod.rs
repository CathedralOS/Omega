//! Git resolution tests that require private acquisition and issuance seams.

use super::{issuance::*, network::*, repository::*};
use crate::custody::tree::*;
use crate::error::SourceResolveError;
use crate::git::cache::{
    creation::{create_git_cache_entry, parse_git_remote_object_format},
    identity::*,
};
use crate::git::executable::executor::test_system_git_executor;
use crate::git::request::*;
use crate::git::snapshot::permissions::*;
use crate::identity::*;
use crate::limits::*;
use crate::local::capture::open_absolute_directory_nofollow;
use crate::observations::{
    accounting::{git_resolution_captured_output_ceiling, git_resolution_network_transfer_ceiling},
    execution::*,
    receipt::*,
    resolution::*,
    resolved::*,
    storage::*,
};
use crate::test_support::*;
use omega_resolver_execution::ResolverExecutionPhase;
use std::process::Command;

mod fetch;
mod resolution;
