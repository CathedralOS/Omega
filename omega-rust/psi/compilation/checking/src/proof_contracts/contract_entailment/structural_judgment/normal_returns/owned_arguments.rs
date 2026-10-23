//! Ordinary calls retain owned inline cells independently of referent isolation.
//! Both entry preconditions and normal-return bindings use this admission;
//! post-call preservation remains the separate call-frame decision.

use super::{StructuralJudge, StructuralTerm, TypeReferenceHandle};

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn owned_argument_observation(
        &self,
        declared: TypeReferenceHandle,
        value: &StructuralTerm,
    ) -> Option<StructuralTerm> {
        if !declared.is_valid() || crate::reference_shell(self.program, declared).is_some() {
            return None;
        }
        if self
            .frame_resolver()
            .is_some_and(|frames| frames.proof_value_is_caller_isolated(declared))
        {
            return Some(value.clone());
        }
        Some(
            super::super::super::structural_terms::owned_projection_observation(
                self.program,
                declared,
                value.clone(),
            ),
        )
    }
}
