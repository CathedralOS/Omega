//! Product optimization requests and subtractive release overlays.
//! Native programs carry the settled selections; they never interpret these requests.
#![forbid(unsafe_code)]
mod rollback;
mod report_request;
pub use rollback::{OptimizationRollback, OptimizationRollbackInputError,
    OptimizationRollbackReceipt, OptimizationRollbackSettlement};
pub use report_request::OptimizationReportRequest;
use optimization_core::{Optimization, OptimizationSelections};
