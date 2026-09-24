//! The one result-bearing return plan a checked machine carries.
//!
//! The return rosters remain separate tables because each retains the
//! structural-type roster its lowering emits and outside readers iterate them
//! by field. This view names the family once, so a consumer asks which return
//! plan a machine lowers instead of walking seven rosters and repeating the
//! same admission beside each. A zero-input payloadless case constructor is
//! not a return family: it is an ordinary Unit-effect body.

use symbols::SymbolHandle;

use crate::checked_trees::flow::FlowFacts;
use crate::checked_trees::flow::terminal::{
    CheckedBoundaryScalarReturnMachinePlan, CheckedClaimFreeAffineStructuralReturnMachinePlan,
    CheckedPayloadlessGuardedCallReturnMachinePlan,
    CheckedSelectedOperatorStructuralScalarReturnMachinePlan, CheckedStructuralReturnMachinePlan,
    CheckedStructuralScalarReturnMachinePlan, CheckedTerminalSignatureEligibility,
    CheckedTraitOperatorScalarReturnMachinePlan,
};

/// The result-bearing plan one machine lowers. Each variant borrows the row
/// of the roster that admitted it. The rosters are filled by independent
/// recognizers over the same machines, so a machine admitted by more than one
/// resolves in this declaration order, which is the producer-family order
/// terminal lowering has always dispatched in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedReturnPlan<'a> {
    /// One scalar result realized by calling a selected boundary operator's
    /// checked realization over the exact structural frontier.
    SelectedOperator(&'a CheckedSelectedOperatorStructuralScalarReturnMachinePlan),
    /// One payloadless structural result saved from a final call and returned
    /// unchanged through its exhaustive identity arms.
    PayloadlessGuardedCall(&'a CheckedPayloadlessGuardedCallReturnMachinePlan),
    /// One scalar result realized by calling a trait-backed fixed-token
    /// realization.
    TraitOperator(&'a CheckedTraitOperatorScalarReturnMachinePlan),
    /// One scalar result computed over a structural frontier, followed by its
    /// post-result cleanup.
    StructuralScalar(&'a CheckedStructuralScalarReturnMachinePlan),
    /// One scalar result returned by a bodyless boundary call that consumes
    /// the structural claim frontier.
    BoundaryScalar(&'a CheckedBoundaryScalarReturnMachinePlan),
    /// One whole linear root transferred to the result with its claim.
    Structural(&'a CheckedStructuralReturnMachinePlan),
    /// One whole claim-free owned-affine parameter returned unchanged.
    ClaimFreeAffine(&'a CheckedClaimFreeAffineStructuralReturnMachinePlan),
}

impl<'a> CheckedReturnPlan<'a> {
    /// The return plan `machine` lowers, if any roster admitted it.
    pub fn for_machine(flow: &'a FlowFacts, machine: SymbolHandle) -> Option<Self> {
        let scalar_returns = &flow.terminal_structural_scalar_returns;
        let structural_returns = &flow.terminal_structural_returns;
        scalar_returns
            .selected_operator_for_machine(machine)
            .map(Self::SelectedOperator)
            .or_else(|| {
                flow.terminal_structural_call_returns
                    .payloadless_guarded_for_machine(machine)
                    .map(Self::PayloadlessGuardedCall)
            })
            .or_else(|| {
                scalar_returns
                    .trait_operator_for_machine(machine)
                    .map(Self::TraitOperator)
            })
            .or_else(|| {
                scalar_returns
                    .for_machine(machine)
                    .map(Self::StructuralScalar)
            })
            .or_else(|| {
                flow.terminal_boundary_scalar_returns
                    .for_machine(machine)
                    .map(Self::BoundaryScalar)
            })
            .or_else(|| {
                structural_returns
                    .for_machine(machine)
                    .map(Self::Structural)
            })
            .or_else(|| {
                structural_returns
                    .claim_free_affine_for_machine(machine)
                    .map(Self::ClaimFreeAffine)
            })
    }

    /// The selected machine this plan returns from.
    pub fn machine(self) -> SymbolHandle {
        match self {
            Self::SelectedOperator(plan) => plan.machine,
            Self::PayloadlessGuardedCall(plan) => plan.machine,
            Self::TraitOperator(plan) => plan.machine,
            Self::StructuralScalar(plan) => plan.machine,
            Self::BoundaryScalar(plan) => plan.machine,
            Self::Structural(plan) => plan.machine,
            Self::ClaimFreeAffine(plan) => plan.machine,
        }
    }

    /// The checked machine whose body realizes this return through one
    /// call, when the plan is such a call rather than an authored body.
    pub fn realization_machine(self) -> Option<SymbolHandle> {
        match self {
            Self::SelectedOperator(plan) => Some(plan.realization_machine),
            Self::TraitOperator(plan) => Some(plan.realization_machine),
            Self::PayloadlessGuardedCall(plan) => Some(plan.target_machine),
            Self::StructuralScalar(_)
            | Self::BoundaryScalar(_)
            | Self::Structural(_)
            | Self::ClaimFreeAffine(_) => None,
        }
    }

    /// The signature a terminal selection must carry to lower this plan:
    /// attached when the plan retains its owner, free otherwise. The
    /// operator-realized kinds constrain their realization instead of the
    /// caller's selection and return `None`.
    pub fn expected_signature(self) -> Option<CheckedTerminalSignatureEligibility> {
        let attached = match self {
            Self::SelectedOperator(_) | Self::TraitOperator(_) => return None,
            Self::StructuralScalar(plan) => plan.attachment_type_identity.is_some(),
            Self::ClaimFreeAffine(plan) => plan.attachment_type_identity.is_some(),
            Self::PayloadlessGuardedCall(_) | Self::BoundaryScalar(_) | Self::Structural(_) => true,
        };
        Some(if attached {
            CheckedTerminalSignatureEligibility::Attached
        } else {
            CheckedTerminalSignatureEligibility::Eligible
        })
    }
}

#[cfg(test)]
mod tests {
    use language_semantics::Multiplicity;
    use symbols::SymbolHandle;

    use super::CheckedReturnPlan;
    use crate::checked_trees::flow::FlowFacts;
    use crate::checked_trees::flow::terminal::{
        CheckedClaimFreeAffineStructuralReturnMachinePlan,
        CheckedPayloadlessGuardedCallReturnMachinePlan, CheckedStructuralAccess,
        CheckedStructuralResultPlan, CheckedTerminalSignatureEligibility,
        CheckedUnitCallCoordinate, CheckedUnitStructuralParameterPlan,
    };

    fn result() -> CheckedStructuralResultPlan {
        CheckedStructuralResultPlan {
            type_identity: "Outcome".to_owned(),
            multiplicity: Multiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }
    }

    fn claim_free_affine(
        machine: SymbolHandle,
        attachment_type_identity: Option<String>,
    ) -> CheckedClaimFreeAffineStructuralReturnMachinePlan {
        CheckedClaimFreeAffineStructuralReturnMachinePlan {
            machine,
            state: SymbolHandle::from_arena_index(9),
            attachment_type_identity,
            structural_parameter: CheckedUnitStructuralParameterPlan {
                position: 0,
                is_self: false,
                type_identity: "Outcome".to_owned(),
                multiplicity: Multiplicity::Affine,
                access: CheckedStructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                fused_service_erasure: None,
            },
            scalar_parameters: Vec::new(),
            result: result(),
            return_statement_ordinal: 0,
        }
    }

    #[test]
    fn the_machine_resolves_to_its_one_roster_row_and_the_signature_its_attachment_implies() {
        let attached = SymbolHandle::from_arena_index(1);
        let free = SymbolHandle::from_arena_index(2);
        let mut flow = FlowFacts::default();
        flow.terminal_structural_returns
            .claim_free_affine_machines
            .push(claim_free_affine(attached, Some("Owner".to_owned())));
        flow.terminal_structural_returns
            .claim_free_affine_machines
            .push(claim_free_affine(free, None));

        let plan = CheckedReturnPlan::for_machine(&flow, attached).expect("attached plan");
        assert!(matches!(plan, CheckedReturnPlan::ClaimFreeAffine(_)));
        assert_eq!(plan.machine(), attached);
        assert_eq!(plan.realization_machine(), None);
        assert_eq!(
            plan.expected_signature(),
            Some(CheckedTerminalSignatureEligibility::Attached)
        );

        let plan = CheckedReturnPlan::for_machine(&flow, free).expect("free plan");
        assert!(matches!(plan, CheckedReturnPlan::ClaimFreeAffine(_)));
        assert_eq!(
            plan.expected_signature(),
            Some(CheckedTerminalSignatureEligibility::Eligible)
        );

        assert_eq!(
            CheckedReturnPlan::for_machine(&flow, SymbolHandle::from_arena_index(3)),
            None
        );
    }

    #[test]
    fn a_machine_in_two_rosters_resolves_in_declaration_order() {
        let machine = SymbolHandle::from_arena_index(1);
        let mut flow = FlowFacts::default();
        flow.terminal_structural_returns
            .claim_free_affine_machines
            .push(claim_free_affine(machine, Some("Owner".to_owned())));
        flow.terminal_structural_call_returns
            .payloadless_guarded_machines
            .push(CheckedPayloadlessGuardedCallReturnMachinePlan {
                machine,
                state: SymbolHandle::from_arena_index(9),
                attachment_type_identity: "Owner".to_owned(),
                result: result(),
                call: CheckedUnitCallCoordinate {
                    statement_index: 0,
                    call_ordinal: 0,
                },
                target_machine: SymbolHandle::from_arena_index(2),
                target_state: SymbolHandle::from_arena_index(10),
                selected_evidence: Vec::new(),
            });

        let plan = CheckedReturnPlan::for_machine(&flow, machine).expect("plan");
        assert!(matches!(plan, CheckedReturnPlan::PayloadlessGuardedCall(_)));
        assert_eq!(
            plan.realization_machine(),
            Some(SymbolHandle::from_arena_index(2))
        );
    }
}
