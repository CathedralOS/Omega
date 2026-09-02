//! Optimizer module role: executable entrance. AArch64 same-view-copy pipeline custody.
//!
//! Source-lineage leaves recover the exact selected/liveness roots. The
//! execution leaf alone dispatches exact machine rules and authenticates their
//! shared disposition carrier.

mod baseline_source;
mod execution;
mod model;
mod selected_lowering_source;

pub use baseline_source::*;
pub use model::*;
pub use selected_lowering_source::*;

use execution::{stage_with_inputs, validate_with_inputs};
