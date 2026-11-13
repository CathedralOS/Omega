//! Target execution records retained after actual provider settlement.
//!
//! Source-based settlement depended on the retired checker. The compiler now
//! refuses that connection until the lowered route supplies its authority;
//! these records and readers neither infer nor manufacture that authority.

#![forbid(unsafe_code)]

mod intrinsic_review;
mod selected_execution;

pub use intrinsic_review::ResolvedAcceptedSemanticBinding;
pub use selected_execution::SelectedExecution;
