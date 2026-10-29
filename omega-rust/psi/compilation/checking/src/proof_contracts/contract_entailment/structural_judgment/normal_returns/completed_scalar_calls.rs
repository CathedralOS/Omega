//! Producer-admitted normal-call identities at the integer projection frontier.
//!
//! Public equalities and completed-call denotations remain ordered facts. Only
//! the latter records that the actual call and its eager operands returned;
//! an arbitrary application/result type supplies no such observation.

use super::super::{StructuralJudge, StructuralTerm};
use typed_trees::types::PrimitiveType;

#[derive(Clone, Debug)]
pub(in crate::proof_contracts::contract_entailment::structural_judgment) struct CapturedResultEquation
{
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) left: StructuralTerm,
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) right: StructuralTerm,
    completed_call_identity: bool,
}

impl CapturedResultEquation {
    pub(super) fn public((left, right): (StructuralTerm, StructuralTerm)) -> Self {
        Self {
            left,
            right,
            completed_call_identity: false,
        }
    }

    /// Called only after the actual normal-return/repeatability admission.
    /// A source-view arithmetic recipe is still a public equation, not an
    /// opaque call leaf: every explicit child keeps its formation obligation.
    pub(super) fn completed_call(left: StructuralTerm, right: StructuralTerm) -> Self {
        let completed_call_identity = matches!(right, StructuralTerm::Application { .. });
        Self {
            left,
            right,
            completed_call_identity,
        }
    }
}

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn completed_integer_call(
        &self,
        value: &StructuralTerm,
    ) -> Option<(StructuralTerm, StructuralTerm, PrimitiveType)> {
        // A total-computation query owes the evaluation itself, even if an
        // earlier runtime normal return supplied an equal mathematical value.
        if self.require_closed_computation || !self.runtime_body_values || self.case_facts_only {
            return None;
        }
        let mut selected = None;
        for equation in self.captured_result_equations.iter() {
            if !equation.completed_call_identity {
                continue;
            }
            let StructuralTerm::CapturedValue { result_type, .. } = &equation.left else {
                continue;
            };
            let Some(primitive) = self.program.primitive_type_reference(*result_type) else {
                continue;
            };
            if !primitive.accepts_integer_literal()
                || !super::super::super::structural_terms::scalar_operand_is_observed(
                    self.program,
                    *result_type,
                    value,
                )
            {
                continue;
            }
            let Some((captured, policy)) =
                self.completed_scalar_observation(&equation.left, primitive)
            else {
                continue;
            };
            if !captured.same_resolution_payload(&equation.left) {
                continue;
            }
            let same_capture = self
                .completed_scalar_observation(value, primitive)
                .is_some_and(|(observed, _)| observed.same_resolution_payload(&captured));
            if !same_capture && !equation.right.same_resolution_payload(value) {
                continue;
            }
            let Some(denotation) = self.single_completed_call_denotation(&captured) else {
                continue;
            };
            selected = Some((captured, denotation, primitive, policy));
            break;
        }
        let (original, denotation, primitive, policy) = selected?;
        let mut canonical = None;
        // Every admitted occurrence of this exact denotation shares one value
        // namespace. A mixed or ambiguous association keeps the general route
        // for the entire class; do not leave one capture opaque while a peer
        // resolves through a different executable/source identity.
        for equation in self.captured_result_equations.iter().filter(|equation| {
            equation.completed_call_identity && equation.right.same_resolution_payload(denotation)
        }) {
            let (captured, candidate_policy) =
                self.completed_scalar_observation(&equation.left, primitive)?;
            if !captured.same_resolution_payload(&equation.left) || candidate_policy != policy {
                return None;
            }
            let candidate_denotation = self.single_completed_call_denotation(&captured)?;
            if !candidate_denotation.same_resolution_payload(denotation) {
                return None;
            }
            // Explicit public recipes retain the complete original projection
            // path, including any later occurrence's stronger literal facts.
            // Decline the whole class rather than mix opaque and expanded peers.
            if !self
                .completed_integer_call_description(&captured)
                .same_resolution_payload(&captured)
            {
                return None;
            }
            canonical.get_or_insert(captured);
        }
        canonical.map(|canonical| (original, canonical, primitive))
    }

    /// Distinct executable/source identities can require a larger equivalence
    /// closure. This optional shortcut declines them; the unchanged resolver
    /// still supports that operation. Repeated identical rows are unambiguous.
    fn single_completed_call_denotation(
        &self,
        captured: &StructuralTerm,
    ) -> Option<&StructuralTerm> {
        let mut denotation: Option<&StructuralTerm> = None;
        for equation in self.captured_result_equations.iter().filter(|equation| {
            equation.completed_call_identity && equation.left.same_resolution_payload(captured)
        }) {
            if let Some(previous) = denotation {
                if !previous.same_resolution_payload(&equation.right) {
                    return None;
                }
            } else {
                denotation = Some(&equation.right);
            }
        }
        denotation
    }

    /// Keep a public computed-result equation ahead of the synthetic call
    /// identity. Return its raw tree, so arithmetic projection still visits
    /// every child instead of normalizing away a partial operation.
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn completed_integer_call_description(
        &self,
        captured: &StructuralTerm,
    ) -> StructuralTerm {
        self.completed_value_identity_at(
            captured.clone(),
            0,
            self.captured_result_equations
                .iter()
                .filter(|equation| !equation.completed_call_identity),
        )
    }
}
