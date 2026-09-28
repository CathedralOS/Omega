//! Optimizer module role: stage decision record. Deliberate rewrite refusals.
//!
//! A proof-bearing module keeps a scalar rewrite only while the reconstructed
//! proof question survives it verbatim. Until proof-context transport lets a
//! question move with the rewrite, `copy_propagation` and
//! `dead_scalar_elimination` refuse a proposal the independent validator
//! rejects at that boundary and return the module as authored. The refusal is
//! an optimization decision, not an error: the stage result lists it so a
//! caller can tell a pass that found nothing to rewrite from a pass that found
//! rewrites and deliberately withheld them.
//!
//! The record is producer-side explanation. It is not part of the canonical
//! execution record encoded at publication and carries no trust: the
//! published module is identical to the input either way, and the receiver
//! re-decides every proof question.

use optimization::PsiOptimization;

/// Why a selected pass withheld the rewrite it proposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsiRewriteRefusalReason {
    /// The independent validator reconstructed a different proof question
    /// from the proposed module: a retained invariant or obligation names a
    /// value the rewrite substituted or removed.
    ChangedProofQuestion,
    /// The proposed module no longer passes optimizable-module validation in
    /// a proof-bearing input: proof context the rewrite cannot carry refers
    /// to a value the proposal removed.
    ProposalLeftProofContextDangling,
}

/// One pass's deliberate refusal of its complete proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PsiRewriteRefusal {
    optimization: PsiOptimization,
    reason: PsiRewriteRefusalReason,
}

impl PsiRewriteRefusal {
    pub(crate) const fn new(
        optimization: PsiOptimization,
        reason: PsiRewriteRefusalReason,
    ) -> Self {
        Self {
            optimization,
            reason,
        }
    }

    pub const fn optimization(&self) -> PsiOptimization {
        self.optimization
    }

    pub const fn reason(&self) -> PsiRewriteRefusalReason {
        self.reason
    }
}
