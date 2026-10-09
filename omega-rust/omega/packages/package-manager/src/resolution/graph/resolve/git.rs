//! Resolve immutable Git package roots.

use super::cache::{SourceCacheLane};
use super::dependencies::{resolve_registered_package_closure};
use super::errors::ResolveGitPackageClosureError;

use package_source::{SourceResolver};
use std::collections::BTreeMap;