// Regression tests for sealed Git command execution, bounded output capture,
// policy flags, and executable launchability/budget enforcement.

#[cfg(unix)]
use super::*;
use super::{policy::*, reconciliation::*};
use crate::error::SourceResolveError;
use crate::git::executable::*;
use crate::git::execution::ResolverExecutionPhase;
use crate::git::request::GitExecutionTransport;
#[cfg(unix)]
use crate::limits::*;
#[cfg(unix)]
use crate::test_support::*;

mod bounded;
mod configuration;
mod executable;
