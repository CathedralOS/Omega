// Git source acquisition tests organized by resolver invariant, binding
// fixture storage to the selected system Git executable for deterministic runs.

use super::cache::identity::*;
use super::executable::executor::test_system_git_executor;
use super::objects::{batch::*, tree::*, *};
use super::request::*;
use super::resolution::*;
use super::snapshot::*;
#[cfg(unix)]
use super::snapshot_metadata::*;
use super::workspace::*;
use crate::PrimaryGitChoices;
use crate::SourceResolver;
use crate::error::SourceResolveError;
use crate::identity::*;
use crate::limits::*;
use crate::snapshot::permissions::*;
use crate::test_support::*;
#[cfg(unix)]
use crate::tree::capture::resolve_materialized_source;
#[cfg(unix)]
use crate::tree::filesystem::raw_os_bytes;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod exact_revision;
mod exact_workspace;
mod request;
mod root_pin;
mod snapshot;
mod tree_session;
mod validation;
mod workspace;

pub(super) fn fixture_storage(storage_base: &Path, request: &GitSourceRequest) -> SourceResolver {
    let executor =
        test_system_git_executor(request.execution_transport()).expect("select fixture Git");
    SourceResolver::for_hardened_base(
        storage_base,
        PrimaryGitChoices {
            explicit_git: Some(executor.execution_backend.executable()),
            ..PrimaryGitChoices::default()
        },
    )
    .expect("retain resolver storage")
}
