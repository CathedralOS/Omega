//! Resolve immutable Git package roots.

use super::super::reconcile::{
};
use super::cache::{SourceCacheLane};
use super::dependencies::{resolve_registered_package_closure};
use super::errors::ResolveGitPackageClosureError;
use crate::resolution::source::{
};
use package_source::{SourceResolver};
use std::collections::BTreeMap;