//! A Match evaluates its subject once. Its aliases consume that completed
//! value through the exact typed capture/anchor, never by replaying storage.
//! Owned nominal admission is retained at this cut, independently of later
//! source writes; reference descriptors keep their ordinary content epochs.
use super::{StructuralJudge, StructuralTerm, TypeReferenceHandle, ValueScope};
use crate::frozen_program::expression_table;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableMatchExpression, TableSavedSubject, ValueCaptureOwner,
};

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn completed_match_subject(
        &self,
        saved: &TableSavedSubject,
    ) -> Option<StructuralTerm> {
        self.program.saved_value_source(saved)?;
        let capture = expression_table(self.program).value_capture(saved.capture);
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
            expression_table(self.program).expression(expression)
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
        let anchor = expression_table(self.program)
            .value_capture(saved.capture)
            .value_anchor;
        let identity = self.program.normalized_type_identity(reference);
        identity.is_valid()
            && self.completed_nominal_values.iter().any(|row| {
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
        self.retain_match_subject(expression, dispatch, value, |judge, value| {
            let reference = judge.normal_value_type_reference(dispatch.subject, scope)?;
            let owned = crate::reference_shell(judge.program, reference).is_none()
                && crate::field_domains::readable_nominal_definition(judge.program, reference)
                    .is_some();
            let admitted =
                owned && judge.has_admitted_nominal_subject(dispatch.subject, value, reference);
            let completed = if owned {
                let snapshot = judge.capture_unknown_value(dispatch.subject, scope)?;
                judge.intake_equation(snapshot.clone(), value.clone(), 0);
                snapshot
            } else {
                value.clone()
            };
            Some((completed, admitted.then_some(reference)))
        })
    }

    /// Retain a contract's bound subject for its saved-subject uses. Source
    /// formation remains an independent debt; this creates no executed load
    /// or nominal membership.
    pub(super) fn complete_declared_match_subject(
        &mut self,
        expression: ExpressionHandle,
        dispatch: &TableMatchExpression,
        value: StructuralTerm,
    ) -> Option<StructuralTerm> {
        self.retain_match_subject(expression, dispatch, value, |_, value| {
            Some((value.clone(), None))
        })
    }

    fn retain_match_subject(
        &mut self,
        expression: ExpressionHandle,
        dispatch: &TableMatchExpression,
        value: StructuralTerm,
        complete: impl FnOnce(
            &mut Self,
            &StructuralTerm,
        ) -> Option<(StructuralTerm, Option<TypeReferenceHandle>)>,
    ) -> Option<StructuralTerm> {
        let table = expression_table(self.program);
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
        let (completed, admitted) = complete(self, &value)?;
        std::sync::Arc::make_mut(&mut self.normal_results)
            .push((capture.value_anchor, completed.clone()));
        if let Some(reference) = admitted {
            std::sync::Arc::make_mut(&mut self.completed_nominal_values).push(
                super::super::CompletedNominalValue {
                    anchor: capture.value_anchor,
                    reference,
                    value: completed.clone(),
                    observed_value: value,
                },
            );
        }
        Some(completed)
    }

    pub(super) fn retire_match_subject(&mut self, dispatch: &TableMatchExpression) {
        if !expression_table(self.program).contains_value_capture(dispatch.capture) {
            return;
        }
        let anchor = expression_table(self.program)
            .value_capture(dispatch.capture)
            .value_anchor;
        std::sync::Arc::make_mut(&mut self.normal_results)
            .retain(|(candidate, _)| *candidate != anchor);
        std::sync::Arc::make_mut(&mut self.completed_nominal_values)
            .retain(|row| row.anchor != anchor);
    }
}
