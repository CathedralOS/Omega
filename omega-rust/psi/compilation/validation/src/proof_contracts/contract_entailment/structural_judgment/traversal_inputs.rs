//! Traversal adds exact dynamic observers and theory scope to resolver inputs.

use super::resolution_inputs::{InputOwner, ResolutionInputs};
use super::{StructuralJudge, SymbolHandle};

pub(in crate::proof_contracts::contract_entailment) struct StructuralTraversalInputs<'program> {
    resolution: ResolutionInputs<'program>,
    specification_terms: bool,
    returned_field_trace: Option<InputOwner<'program>>,
    active_application: Option<InputOwner<'program>>,
    resolution_observations: Option<InputOwner<'program>>,
    ring_licenses: Vec<SymbolHandle>,
    semiring_licenses: Vec<(SymbolHandle, SymbolHandle)>,
}

impl StructuralTraversalInputs<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn same_inputs(
        &self,
        other: &Self,
    ) -> bool {
        self.resolution.same_inputs(&other.resolution)
            && self.specification_terms == other.specification_terms
            && self.returned_field_trace == other.returned_field_trace
            && self.active_application == other.active_application
            && self.resolution_observations == other.resolution_observations
            && self.ring_licenses == other.ring_licenses
            && self.semiring_licenses == other.semiring_licenses
    }
}

impl<'program> StructuralJudge<'program> {
    pub(in crate::proof_contracts::contract_entailment) fn traversal_inputs(
        &self,
    ) -> StructuralTraversalInputs<'program> {
        StructuralTraversalInputs {
            resolution: self.resolution_inputs(),
            specification_terms: self.specification_terms,
            returned_field_trace: self
                .query_state
                .returned_field_trace
                .as_ref()
                .map(InputOwner::new),
            active_application: self.active_application.as_ref().map(InputOwner::new),
            resolution_observations: self.resolution_observations.as_ref().map(InputOwner::new),
            ring_licenses: self
                .ring_licenses
                .iter()
                .map(|license| license.add_machine)
                .collect(),
            semiring_licenses: self
                .semiring_licenses
                .iter()
                .map(|license| (license.add_machine, license.mul_machine))
                .collect(),
        }
    }
}
