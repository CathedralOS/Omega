//! Optimizer module role: executable entrance. AArch64 same-view-copy pipeline custody.
//!
//! The allocation boundary supplies current selected/liveness facts. Execution
//! dispatches exact machine rules and validates their shared disposition carrier.

mod execution;
mod model;
mod source;

pub use model::*;
pub use source::*;

use execution::{stage_with_inputs, validate_with_inputs};
