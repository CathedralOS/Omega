//! Command execution regression tests grouped by resolver behavior.

use super::{capture::*, policy::*, reconciliation::*};
use crate::package_source::error::SourceResolveError;
#[cfg(unix)]
use crate::package_source::git::executable::budget::*;
use crate::package_source::git::executable::executor::*;
use crate::package_source::git::request::GitExecutionTransport;
use crate::package_source::limits::*;
use crate::package_source::test_support::*;
use crate::resolver_execution::ResolverExecutionPhase;

mod bounded;
mod configuration;
mod executable;
mod lifecycle;
