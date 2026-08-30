//! Command execution regression tests grouped by resolver behavior.

use super::{capture::*, command::*, invocation::*, reconciliation::*};
use crate::error::SourceResolveError;
use crate::git::executable::{budget::*, executor::*};
use crate::git::request::GitExecutionTransport;
use crate::limits::*;
use crate::observations::accounting::git_resolution_captured_output_ceiling;
use crate::test_support::*;
use omega_resolver_execution::ResolverExecutionPhase;

mod bounded;
mod configuration;
mod executable;
