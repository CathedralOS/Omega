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
        if self.require_closed_computation
            || !self.runtime_body_values
            || self.case_facts_only
            || self.has_resolution_dependency_collector()
        {
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
            if !same_capture && !self.same_completed_denotation(&equation.right, value) {
                continue;
            }
            let Some(denotation) = self.single_completed_call_denotation(&captured) else {
                continue;
            };
            selected = Some((captured, denotation, primitive, policy));
            break;
        }
        let (original, denotation, primitive, policy) = selected?;
        // A composite value rewrite may have already unfolded a call
        // during intake, erasing the marker that would relate it to this
        // class. Keep the entire numeric context in the general namespace
        // whenever such a value fact exists. Do not build a second rewrite
        // normalization graph. Integer comparisons and Boolean operators
        // remain predicates; Application predicates conservatively decline
        // retention too, since their result need not be projected here.
        if self
            .rewrites
            .iter()
            .any(|(pattern, _)| match pattern.semantic_value() {
                StructuralTerm::ScalarBinary {
                    operator, meaning, ..
                } => typed_trees::operator::builtin_integer_result_carrier(
                    *operator,
                    meaning.map(|(primitive, _)| primitive),
                )
                .is_some(),
                StructuralTerm::MathematicalBinary { operator, .. } => !operator.is_comparison(),
                StructuralTerm::Application { .. }
                | StructuralTerm::CallProjection { .. }
                | StructuralTerm::IndexedValue { .. }
                | StructuralTerm::Projection { .. }
                | StructuralTerm::CollectionMeasure { .. }
                | StructuralTerm::ScalarRepresentation { .. } => true,
                _ => false,
            })
        {
            return None;
        }
        let mut canonical = None;
        // Every admitted occurrence of this exact denotation shares one value
        // namespace. A mixed or ambiguous association keeps the general route
        // for the entire class; do not leave one capture opaque while a peer
        // resolves through a different executable/source identity.
        for equation in self.captured_result_equations.iter().filter(|equation| {
            equation.completed_call_identity
                && self.same_completed_denotation(&equation.right, denotation)
        }) {
            let (captured, candidate_policy) =
                self.completed_scalar_observation(&equation.left, primitive)?;
            if !captured.same_resolution_payload(&equation.left) || candidate_policy != policy {
                return None;
            }
            let candidate_denotation = self.single_completed_call_denotation(&captured)?;
            if !self.same_completed_denotation(candidate_denotation, denotation) {
                return None;
            }
            // Current path facts need not be in the public-equation roster.
            // Preserve any nonidentity capture rewrite and any Application
            // rewrite through the unchanged general route for the whole class.
            let canonical_denotation = self.canonical_completed_denotation(candidate_denotation);
            if self
                .rewrites
                .find(&captured)
                .is_some_and(|(_, replacement)| {
                    !replacement.same_resolution_payload(candidate_denotation)
                        && !replacement.same_resolution_payload(&canonical_denotation)
                })
                || self.rewrites.find(candidate_denotation).is_some()
                || self.rewrites.find(&canonical_denotation).is_some()
            {
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

    /// Normalize only the independently equivalent full result recipe. No
    /// argument, target, selection or observation payload is erased here.
    fn canonical_completed_denotation(&self, term: &StructuralTerm) -> StructuralTerm {
        let mut term = term.clone();
        if let StructuralTerm::Application { result_type, .. } = &mut term {
            *result_type = super::super::super::specification_calls::canonical_result_reference(
                self.program,
                *result_type,
            );
        }
        term
    }

    fn same_completed_denotation(&self, left: &StructuralTerm, right: &StructuralTerm) -> bool {
        if left.same_resolution_payload(right) {
            return true;
        }
        match (left, right) {
            (
                StructuralTerm::Application {
                    target: left_target,
                    arguments: left_arguments,
                    ..
                },
                StructuralTerm::Application {
                    target: right_target,
                    arguments: right_arguments,
                    ..
                },
            ) if left_target == right_target && left_arguments.len() == right_arguments.len() => {
                self.canonical_completed_denotation(left)
                    .same_resolution_payload(&self.canonical_completed_denotation(right))
            }
            _ => false,
        }
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
                if !self.same_completed_denotation(previous, &equation.right) {
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
