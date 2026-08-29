#![forbid(unsafe_code)]

//! Abstract-operation lowering into target-specific operation plans.
//!
//! Enter `lowering/mod.rs` for the validated settlement-to-function lowering
//! join, then descend by result family and semantic responsibility.

mod lowering;
mod model;

pub use lowering::{
    lower_ranked_to_target_operations, lower_to_target_operations,
    lower_to_target_operations_with_provider_executions,
    lower_to_target_operations_with_provider_executions_and_installation,
};
pub use model::{AdmittedBoundarySettlement, LoweringError};

#[cfg(test)]
mod tests;
