//! Completed equations retain the operation that established them.
//! Whole-data Id, scalar substitution and repeatable-call identity grant
//! different authority to their consumers.

use super::super::StructuralTerm;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CapturedEquationMeaning {
    PublicScalar,
    CompletedCall,
    DataIdentity,
}

#[derive(Clone, Debug)]
pub(in crate::proof_contracts::contract_entailment::structural_judgment) struct CapturedResultEquation
{
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) left: StructuralTerm,
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) right: StructuralTerm,
    meaning: CapturedEquationMeaning,
}

impl CapturedResultEquation {
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn is_data_identity(
        &self,
    ) -> bool {
        self.meaning == CapturedEquationMeaning::DataIdentity
    }

    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn is_completed_call_identity(
        &self,
    ) -> bool {
        self.meaning == CapturedEquationMeaning::CompletedCall
    }

    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn public_scalar_replacement(
        &self,
        observed: &StructuralTerm,
    ) -> Option<&StructuralTerm> {
        if self.meaning != CapturedEquationMeaning::PublicScalar {
            return None;
        }
        if self.left.same_resolution_payload(observed) {
            Some(&self.right)
        } else if self.right.same_resolution_payload(observed) {
            Some(&self.left)
        } else {
            None
        }
    }

    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn retains_completed_identity(
        &self,
        captured: &StructuralTerm,
        description: &StructuralTerm,
    ) -> bool {
        self.is_completed_call_identity()
            && self.left.same_resolution_payload(captured)
            && self.right.same_resolution_payload(description)
    }

    pub(super) fn public((left, right): (StructuralTerm, StructuralTerm)) -> Self {
        Self {
            left,
            right,
            meaning: CapturedEquationMeaning::PublicScalar,
        }
    }

    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn data_identity(
        (left, right): (StructuralTerm, StructuralTerm),
    ) -> Self {
        Self {
            left,
            right,
            meaning: CapturedEquationMeaning::DataIdentity,
        }
    }

    /// Called only after the actual normal-return/repeatability admission.
    /// A source-view arithmetic recipe is still a public equation, not an
    /// opaque call leaf: every explicit child keeps its formation obligation.
    pub(super) fn completed_call(left: StructuralTerm, right: StructuralTerm) -> Self {
        let meaning = if matches!(right, StructuralTerm::Application { .. }) {
            CapturedEquationMeaning::CompletedCall
        } else {
            CapturedEquationMeaning::PublicScalar
        };
        Self {
            left,
            right,
            meaning,
        }
    }
}
