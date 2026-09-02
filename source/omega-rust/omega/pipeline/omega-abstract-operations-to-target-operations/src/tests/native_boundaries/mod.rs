//! Optimizer module role: stage group. Native-boundary lowering coverage.

use super::*;

mod installed_providers;
mod linux_exit_group;
mod linux_write_and_exit;
mod scalar_call_and_exit;
