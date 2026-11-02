//! Completed nominal values retain formation at their producing occurrence.
//! Reads still authenticate current storage before consuming these receipts;
//! an equal constructor spelling or receiving annotation supplies no validity.
use super::{ExpressionHandle, StructuralJudge, StructuralTerm, TypeReferenceHandle, ValueScope};

impl StructuralJudge<'_> {
    /// The caller owns independent admission at the completed operation cut.
    /// A unique snapshot carries that validity through local/transition homes;
    /// its denotation remains available without granting equal values admission.
    pub(super) fn retain_completed_nominal_value(
        &mut self,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
        observed: StructuralTerm,
        scope: ValueScope<'_>,
    ) -> Option<StructuralTerm> {
        let value = self.capture_unknown_value(expression, scope)?;
        self.assume_completed_value_equation(reference, value.clone(), observed.clone());
        std::sync::Arc::make_mut(&mut self.completed_nominal_values).push(
            super::super::CompletedNominalValue {
                anchor: expression,
                reference,
                value: value.clone(),
                observed_value: observed,
            },
        );
        Some(value)
    }

    /// Recover the denotation of this exact completed nominal value.
    /// This receipt is produced by subject evaluation, not by value-equation
    /// rewriting. Storage fences still govern obtaining the source observation.
    pub(in crate::proof_contracts::contract_entailment) fn completed_nominal_observation(
        &self,
        value: &StructuralTerm,
    ) -> StructuralTerm {
        let mut observed = value.clone();
        for _ in 0..self.completed_nominal_values.len() {
            let Some(identity) = observed.observation_identity_key() else {
                break;
            };
            let Some(capture) = self.completed_nominal_values.iter().rev().find(|capture| {
                capture.value.observation_identity_key().as_ref() == Some(&identity)
            }) else {
                break;
            };
            observed = capture.observed_value.clone();
        }
        observed
    }
}
