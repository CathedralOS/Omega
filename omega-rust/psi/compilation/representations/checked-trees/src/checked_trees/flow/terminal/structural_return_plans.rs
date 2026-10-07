//! Structural return plans: claim-free affine identity returns.

use crate::checked_trees::flow::terminal::{
    CheckedStructuralResultPlan, CheckedStructuralScalarParameterPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralTypePlan,
};
use symbols::SymbolHandle;

/// Source-handle-free checked plans for whole claim-free owned-affine
/// identity returns used by selected fixed-token realizations. A claim-bearing
/// result, a guarded call result and a payload-less case constructor have no
/// lane here: each is an ordinary body the general Unit closure lowers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralReturnPlans {
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    pub claim_free_affine_machines: Vec<CheckedClaimFreeAffineStructuralReturnMachinePlan>,
}

impl CheckedStructuralReturnPlans {
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
