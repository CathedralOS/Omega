//! Optimizer module role: stage decision record. Rewrites a pass proposed and
//! then withheld.
//!
//! A pass that finds nothing to rewrite and a pass that finds a rewrite and
//! declines to keep it are different outcomes, and a report that shows neither
//! cannot tell them apart. This record is the second one: the optimization
//! that proposed, and the reason code its own validator returned.
//!
//! It is the [skipped decision](../../../../../../wiki/spec/build/optimizations.md)
//! row in the shape reports already project, so it reuses
//! `OptimizationReasonCode` rather than introducing a second refusal
//! vocabulary beside it. Producers map their internal refusal reasons onto
//! that vocabulary at the boundary where the record is built.
//!
//! The record is producer-side explanation carrying no trust. The published
//! module is identical either way — a withheld rewrite changed nothing — so
//! this never enters the canonical execution record, and a receiver re-decides
//! every question for itself.

use crate::OptimizationReasonCode;
use optimization::PsiOptimization;

/// One pass's proposal, withheld, with the reason its validator gave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WithheldRewrite {
    optimization: PsiOptimization,
    reason: OptimizationReasonCode,
}

impl WithheldRewrite {
    pub const fn new(optimization: PsiOptimization, reason: OptimizationReasonCode) -> Self {
        Self {
            optimization,
            reason,
        }
    }

    /// The exact pass that proposed the rewrite. Reports name exact rules.
    pub const fn optimization(&self) -> PsiOptimization {
        self.optimization
    }

    /// Why the proposal was withheld, in the shared decision vocabulary.
    pub const fn reason(&self) -> OptimizationReasonCode {
        self.reason
    }
}
