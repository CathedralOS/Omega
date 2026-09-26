//! Git resolution tests that require private acquisition and issuance seams.

use super::{issuance::*, network::*, repository::*};
use crate::package_source::custody::tree::*;
use crate::package_source::error::SourceResolveError;
use crate::package_source::git::cache::{
    creation::{create_git_cache_entry, parse_git_remote_object_format},
    identity::*,
};
use crate::package_source::git::executable::executor::test_system_git_executor;
use crate::package_source::git::request::*;
use crate::package_source::identity::*;
use crate::package_source::limits::*;
use crate::package_source::observations::resolved::*;
use crate::package_source::snapshot::permissions::*;
use crate::package_source::test_support::*;
use crate::package_source::tree::filesystem::open_absolute_directory_nofollow;
use std::process::Command;

mod fetch;
mod resolution;
