//! Product optimization requests and subtractive release overlays.
//! Native programs carry the settled selections; they never interpret these requests.
#![forbid(unsafe_code)]
mod report_request;
mod rollback;
use optimization_core::{Optimization, OptimizationSelections};
pub use report_request::OptimizationReportRequest;
pub use rollback::{
    OptimizationRollback, OptimizationRollbackInputError, OptimizationRollbackReceipt,
    OptimizationRollbackSettlement,
};
