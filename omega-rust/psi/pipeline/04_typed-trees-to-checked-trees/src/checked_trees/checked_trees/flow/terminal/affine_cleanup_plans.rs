//! Affine cleanup plans: partial and nominal affine discards, caller
//! requirements, claim transfers and call custody.

use crate::checked_trees::checked_trees::flow::terminal::{
    CheckedUnitEffectMachinePlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypePlan,
};
use language_semantics::SemanticDomainId;
use symbols::SymbolHandle;

/// One claim-free affine structural place disposed on the normal edge selected
/// by its containing plan. An empty path denotes the whole root; a nonempty path
/// denotes a retained residual subtree. The root is a structural parameter or
/// call-result binding, with canonical semantic path identity. Other source
/// kinds do not yet admit this cleanup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitPartialAffineDiscardPlan {
    pub source: CheckedUnitStructuralArgumentSourcePlan,
    pub path: Vec<CheckedUnitStructuralPathSegment>,
    pub type_identity: String,
}

/// Checked-only carrier for direct-record-field partial cleanup. It remains
/// separate from `CheckedUnitEffectPlans` because its return cleanup is
/// path-sensitive rather than a whole-root discard.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedPartialAffineUnitCleanupPlans {
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    pub machines: Vec<CheckedPartialAffineUnitCleanupMachinePlan>,
}

impl CheckedPartialAffineUnitCleanupPlans {
    pub fn for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedPartialAffineUnitCleanupMachinePlan> {
        self.machines
            .iter()
            .find(|plan| plan.machine.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPartialAffineUnitCleanupMachinePlan {
    /// Complete ordinary Unit signature/call/return plan. This machine is not
    /// published through `CheckedUnitEffectPlans` while its return cleanup is
    /// still path-sensitive.
    pub machine: CheckedUnitEffectMachinePlan,
    /// Exact maximal residual subtrees after every source-ordered projected
    /// call commits. Pairwise prefix-disjoint paths are grouped recursively at
    /// selected ancestors while retaining reverse declaration order at every
    /// record level.
    pub residual_affine_discards: Vec<CheckedUnitPartialAffineDiscardPlan>,
}

/// Checked-only carrier for the first whole-root nominal-cleanup slice. It is
/// intentionally separate from `CheckedUnitEffectPlans`: a nominal cleanup is
/// executable edge work and must never be reinterpreted as a trivial affine
/// discard by an older terminal producer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedNominalAffineUnitCleanupPlans {
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    pub machines: Vec<CheckedNominalAffineUnitCleanupMachinePlan>,
}

impl CheckedNominalAffineUnitCleanupPlans {
    pub fn for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedNominalAffineUnitCleanupMachinePlan> {
        self.machines
            .iter()
            .find(|plan| plan.machine.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedNominalAffineUnitCleanupMachinePlan {
    /// Exact ordinary Unit signature and return edge. Both trivial-discard
    /// lists are empty; `cleanups` is the complete reverse-parameter-order
    /// disposal list committed by the edge.
    pub machine: CheckedUnitEffectMachinePlan,
    /// Complete canonical direct-Boolean caller requirement set at the sole
    /// return edge. Cleanup-local requirements select subsets of this set by
    /// `source_parameter_index`.
    pub caller_requirements: Vec<CheckedUnitNominalAffineCallerRequirementPlan>,
    pub cleanups: Vec<CheckedUnitNominalAffineCleanupPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitNominalAffineCallerRequirementPlan {
    pub source_parameter_index: u32,
    pub field_identity: String,
    pub expected: bool,
}

/// One whole affine parameter disposed by its exact checked empty nominal
/// cleanup machine. The parameter index is dense in the checked structural
/// signature and the type identity joins the root to the cleanup attachment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitNominalAffineCleanupPlan {
    pub source_parameter_index: u32,
    pub type_identity: String,
    pub cleanup_machine: SymbolHandle,
    pub cleanup_state: SymbolHandle,
    pub cleanup_contract_report_fingerprint: u64,
    /// Source-independent preconditions proved at this exact implicit cleanup
    /// edge. The contextual slice admits a finite canonical set of direct
    /// relevant Boolean fields of the cleanup receiver.
    pub requirements: Vec<CheckedUnitNominalAffineCleanupRequirementPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitNominalAffineCleanupRequirementPlan {
    /// Normalized declaration identity, using the same `#identity`/name
    /// convention as [`CheckedUnitStructuralFieldPlan::identity`].
    pub field_identity: String,
    pub expected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitClaimTransferPlan {
    pub claim_identity: language_semantics::PermissionClaimIdentity,
    /// Dense index into this call's structural argument list. The callee-local
    /// claim identity is reconstructed by terminal verification.
    pub argument_index: u32,
}

/// Ordinary structural result custody is separate from its value binding.
/// These rows preserve existing claims through successful call completion;
/// they never establish fresh authority. An empty record is valid only for
/// an independently checked unqualified, claim-free result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralCallCustodyPlan {
    /// Exact source loan carried by a root reference result. A zero handle
    /// means this call establishes no reference-result resource.
    pub reference_loan: arena::Handle<crate::checked_trees::BorrowLoanFact>,
    pub result_qualifications: Vec<SemanticDomainId>,
    pub claim_transfers: Vec<CheckedUnitClaimTransferPlan>,
    pub returned_claim_transfers: Vec<CheckedStructuralReturnedClaimTransferPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralReturnedClaimTransferPlan {
    pub callee_claim: language_semantics::PermissionClaimIdentity,
    pub caller_claim: language_semantics::PermissionClaimIdentity,
    /// Equal canonical input-relative and result-relative claim path. The
    /// containing call transfers the whole owner, not a projected argument.
    pub path: Vec<CheckedUnitStructuralPathSegment>,
}
