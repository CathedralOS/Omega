//! Command execution regression tests grouped by resolver behavior.

use super::{capture::*, policy::*, reconciliation::*};
use crate::error::SourceResolveError;
use crate::git::executable::executor::*;
use crate::git::request::GitExecutionTransport;
use crate::limits::*;
use crate::test_support::*;
use resolver_execution::ResolverExecutionPhase;

mod bounded;
mod configuration;
mod executable;
