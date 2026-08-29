//! Register-allocation rule-stage map.
//!
//! Each executable phase owns a meaningful entrance and adjacent catalog.
//! Shared target applicability is the only cross-phase rule contract here.

mod allocation_recovery;
mod selected_lowering;

pub use allocation_recovery::*;
pub use selected_lowering::*;

/// Register-allocation rules are architecture-independent. The explicit
/// marker keeps portability in each owning catalog instead of implicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterAllocationRuleTargetApplicability {
    TargetIndependent,
}
