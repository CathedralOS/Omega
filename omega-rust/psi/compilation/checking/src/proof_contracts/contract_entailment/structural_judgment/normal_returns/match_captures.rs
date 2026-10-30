//! A Match evaluates its subject once. Its aliases consume that completed
//! value through the exact typed capture/anchor, never by replaying storage.
//! Owned nominal admission is retained at this cut, independently of later
//! source writes; reference descriptors keep their ordinary content epochs.
use super::{StructuralJudge, StructuralTerm, TypeReferenceHandle, ValueScope};
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableMatchExpression, TableSavedSubject, ValueCaptureOwner,
};

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn completed_match_subject(
        &self,
        saved: &TableSavedSubject,
    ) -> Option<StructuralTerm> {
        self.program.saved_value_source(saved)?;
        let capture = self.program.expression_table.value_capture(saved.capture);
        if !matches!(capture.owner, ValueCaptureOwner::Match) {
            return None;
        }
        self.normal_results
            .iter()
            .rev()
            .find(|(anchor, _)| *anchor == capture.value_anchor)
            .map(|(_, value)| value.clone())
    }

    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn completed_match_nominal_admission(
        &self,
        expression: ExpressionHandle,
        value: &StructuralTerm,
        reference: TypeReferenceHandle,
    ) -> bool {
        let ExpressionNode::SavedSubject(saved) =
            self.program.expression_table.expression(expression)
        else {
            return false;
        };
        let Some(bound) = self.completed_match_subject(saved) else {
            return false;
        };
        let Some(key) = value.observation_identity_key() else {
            return false;
        };
        if bound.observation_identity_key().as_ref() != Some(&key) {
            return false;
        }
        let anchor = self
            .program
            .expression_table
            .value_capture(saved.capture)
            .value_anchor;
        let identity = self.program.normalized_type_identity(reference);
        identity.is_valid()
            && self.completed_nominal_captures.iter().any(|row| {
                row.anchor == anchor
                    && row.value.observation_identity_key().as_ref() == Some(&key)
                    && self.program.normalized_type_identity(row.reference) == identity
            })
    }

    pub(super) fn complete_match_subject(
        &mut self,
        expression: ExpressionHandle,
        dispatch: &TableMatchExpression,
        value: StructuralTerm,
        scope: ValueScope<'_>,
    ) -> Option<StructuralTerm> {
        let table = &self.program.expression_table;
        if !dispatch.capture.is_valid() {
            return Some(value);
        }
        if !table.contains_value_capture(dispatch.capture) {
            return None;
        }
        let capture = *table.value_capture(dispatch.capture);
        if !matches!(capture.owner, ValueCaptureOwner::Match)
            || capture.definition != expression
            || capture.subject != dispatch.subject
        {
            return None;
        }
        if !capture.value_anchor.is_valid() {
            return Some(value);
        }
        let ExpressionNode::SavedSubject(saved) = table.expression(capture.value_anchor) else {
            return None;
        };
        if saved.capture != dispatch.capture
            || self.program.saved_value_source(saved) != Some(dispatch.subject)
        {
            return None;
        }
        self.retire_match_subject(dispatch);
        let reference = self.normal_value_type_reference(dispatch.subject, scope)?;
        let owned =
            crate::field_domains::owned_nominal_data_definition(self.program, reference).is_some();
        let admitted =
            owned && self.has_admitted_nominal_subject(dispatch.subject, &value, reference);
        let completed = if owned {
            let snapshot = self.capture_unknown_value(dispatch.subject, scope)?;
            self.intake_equation(snapshot.clone(), value.clone(), 0);
            snapshot
        } else {
            value.clone()
        };
        std::sync::Arc::make_mut(&mut self.normal_results)
            .push((capture.value_anchor, completed.clone()));
        if admitted {
            std::sync::Arc::make_mut(&mut self.completed_nominal_captures).push(
                super::super::CompletedNominalCapture {
                    anchor: capture.value_anchor,
                    reference,
                    value: completed.clone(),
                    observed_value: value,
                },
            );
        }
        Some(completed)
    }

    /// A tag observation follows the exact owned value captured by this Match.
    /// This receipt is produced by subject evaluation, not by value-equation
    /// rewriting. Storage fences still govern obtaining the source observation.
    pub(super) fn completed_match_case_observation(
        &self,
        value: &StructuralTerm,
    ) -> StructuralTerm {
        let mut observed = value.clone();
        for _ in 0..self.completed_nominal_captures.len() {
            let Some(identity) = observed.observation_identity_key() else {
                break;
            };
            let Some(capture) = self
                .completed_nominal_captures
                .iter()
                .rev()
                .find(|capture| {
                    capture.value.observation_identity_key().as_ref() == Some(&identity)
                })
            else {
                break;
            };
            observed = capture.observed_value.clone();
        }
        observed
    }

    pub(super) fn retire_match_subject(&mut self, dispatch: &TableMatchExpression) {
        if !self
            .program
            .expression_table
            .contains_value_capture(dispatch.capture)
        {
            return;
        }
        let anchor = self
            .program
            .expression_table
            .value_capture(dispatch.capture)
            .value_anchor;
        std::sync::Arc::make_mut(&mut self.normal_results)
            .retain(|(candidate, _)| *candidate != anchor);
        std::sync::Arc::make_mut(&mut self.completed_nominal_captures)
            .retain(|row| row.anchor != anchor);
    }
}
