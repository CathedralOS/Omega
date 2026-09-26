//! Structural return plans: whole-root transfers, claim-free affine returns
//! and guarded payloadless call returns with their evidence.

use crate::checked_trees::flow::terminal::{
    CheckedStructuralResultPlan, CheckedStructuralScalarParameterPlan,
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitCallCoordinate, CheckedUnitEntryClaimPlan,
    CheckedUnitStructuralDomainPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralTypePlan,
};
use symbols::SymbolHandle;

/// Source-handle-free checked plans for the bounded structural-result lanes.
/// Claim-bearing machines admit an exact whole-root linear transfer. A
/// zero-input payload-less case constructor has no lane here: it is an
/// ordinary Unit-effect body whose structural result the general Unit closure
/// lowers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralReturnPlans {
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    pub structural_domains: Vec<CheckedUnitStructuralDomainPlan>,
    pub machines: Vec<CheckedStructuralReturnMachinePlan>,
    /// Exact whole, claim-free owned-affine returns used by selected
    /// fixed-token realizations. This stays distinct from the linear
    /// claim-transfer family above: an absent claim is not a degenerate
    /// transferred claim.
    pub claim_free_affine_machines: Vec<CheckedClaimFreeAffineStructuralReturnMachinePlan>,
}

impl CheckedStructuralReturnPlans {
    pub fn for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedStructuralReturnMachinePlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }

    pub fn claim_free_affine_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedClaimFreeAffineStructuralReturnMachinePlan> {
        self.claim_free_affine_machines
            .iter()
            .find(|plan| plan.machine == machine)
    }
}

/// Source-handle-free checked plan for one exact whole owned-affine parameter
/// returned without claims, projections, services, or cleanup. Fixed-width
/// scalar parameters retain their authored positions for mixed ABI planning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedClaimFreeAffineStructuralReturnMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub attachment_type_identity: Option<String>,
    pub structural_parameter: CheckedUnitStructuralParameterPlan,
    pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    pub result: CheckedStructuralResultPlan,
    pub return_statement_ordinal: u32,
}

/// Guarded payloadless result calls retain their erased evidence selectors.
/// Ordinary linear result calls belong to shared statement sequencing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralCallReturnPlans {
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    pub structural_domains: Vec<CheckedUnitStructuralDomainPlan>,
    /// Exact unrestricted payloadless calls whose exhaustive case arms all
    /// return the saved call result unchanged. Proof selectors remain erased
    /// and may bind at most one guarded caller-local evidence term.
    pub payloadless_guarded_machines: Vec<CheckedPayloadlessGuardedCallReturnMachinePlan>,
}

impl CheckedStructuralCallReturnPlans {
    pub fn payloadless_guarded_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedPayloadlessGuardedCallReturnMachinePlan> {
        self.payloadless_guarded_machines
            .iter()
            .find(|plan| plan.machine == machine)
    }
}

/// One guarded payloadless call returned through its exhaustive identity arms.
/// The callee is not a plan of this family: `target_machine` is a zero-input
/// machine on the same attachment returning the same unrestricted sum, and it
/// lowers through its own ordinary Unit-effect plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPayloadlessGuardedCallReturnMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub attachment_type_identity: String,
    pub result: CheckedStructuralResultPlan,
    pub call: CheckedUnitCallCoordinate,
    pub target_machine: SymbolHandle,
    /// The callee's entry state, which the call names.
    pub target_state: SymbolHandle,
    /// Canonically ordered explicitly selected named rows. An empty vector
    /// retains the callee's guarded implications without minting caller terms.
    pub selected_evidence: Vec<CheckedPayloadlessGuardedCallEvidencePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPayloadlessGuardedCallEvidencePlan {
    pub arm_statement_index: u32,
    pub guarantee: arena::Handle<crate::OutcomeSpecificGuaranteeFact>,
    pub selected_term: arena::Handle<crate::CheckedEvidenceTerm>,
    /// True exactly for the bounded whole-result proposition substitution.
    pub substitutes_result: bool,
    /// The one bounded proof-only use of this selected term. Up to two distinct
    /// selected rows may each occupy one dense tail-requirement lane. Runtime
    /// lowering continues to return the saved structural result unchanged.
    pub tail_use: Option<CheckedPayloadlessGuardedCallEvidenceUsePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPayloadlessGuardedCallEvidenceUsePlan {
    pub target_state: SymbolHandle,
    pub input_position: u32,
    pub parameter: arena::Handle<crate::CheckedEvidenceTerm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralReturnMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub attachment_type_identity: String,
    /// Exact structural signature in dense terminal order: one returned linear
    /// root followed by a finite claim-free affine cleanup tail.
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    /// Dense parameter index of the whole root transferred to the result.
    pub returned_parameter_index: u32,
    pub result: CheckedStructuralResultPlan,
    /// Source-handle-free declarations for the exact trivial affine locals
    /// established before the whole-root return, in dense declaration order.
    pub trivial_affine_locals: Vec<CheckedTrivialAffineStructuralLocalPlan>,
    /// Exact local declaration coordinates cleaned before parameter cleanup,
    /// in reverse declaration order.
    pub trivial_affine_local_discard_ordinals: Vec<u32>,
    pub entry_claim: CheckedUnitEntryClaimPlan,
    /// Exact reverse-declaration affine parameter cleanup positions committed
    /// after result materialization.
    pub trivial_affine_discards: Vec<u32>,
    /// Exact identity shared by the entry claim, normalized claim outcome, and
    /// identity-reshuffle fact.
    pub transferred_claim: language_semantics::PermissionClaimIdentity,
}
