//! Command execution regression tests grouped by resolver behavior.

#[cfg(unix)]
use super::invocation::*;
use super::{capture::*, command::*, reconciliation::*};
use crate::error::SourceResolveError;
#[cfg(unix)]
use crate::git::executable::budget::*;
use crate::git::executable::executor::*;
use crate::git::request::GitExecutionTransport;
use crate::limits::*;
use crate::test_support::*;
use resolver_execution::ResolverExecutionPhase;

mod bounded;
mod configuration;
mod executable;
