//! Command execution regression tests grouped by resolver behavior.

use super::{capture::*, command::*, invocation::*, reconciliation::*};
use crate::error::SourceResolveError;
use crate::git::executable::{budget::*, custody::*, executor::*, selection::*};
use crate::git::request::GitExecutionTransport;
use crate::limits::*;
use crate::observations::accounting::{
    git_resolution_captured_output_ceiling, git_resolution_network_transfer_ceiling,
};
use crate::test_support::*;
use omega_resolver_execution::{
    RESOLVER_CONNECT_BROKER_ENVIRONMENT, RESOLVER_CONNECT_HELPER_BASENAME,
    RESOLVER_CONNECT_TARGET_ENVIRONMENT, ResolverExecutionPhase,
    ResolverExecutionRequestedEndpoint,
};

mod bounded;
mod configuration;
mod executable;
mod transport_chain;
