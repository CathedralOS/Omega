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
use crate::identity::*;
use crate::limits::*;
use crate::observations::resolved::*;
use crate::snapshot::permissions::*;
use crate::test_support::*;
use crate::tree::filesystem::open_absolute_directory_nofollow;
use std::process::Command;

mod fetch;
mod resolution;
