//! Optimizer module role: stage group. Native-boundary lowering coverage.

use super::*;

mod hosted_exit_process;
mod installed_providers;
mod linux_write_and_exit;
mod returning_byte_parameter;
mod scalar_call_and_exit;
mod scalar_definitions;
mod unit_graph;
