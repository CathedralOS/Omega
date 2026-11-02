//! Completed nominal values retain formation at their producing occurrence.
//! Reads still authenticate current storage before consuming these receipts;
//! an equal constructor spelling or receiving annotation supplies no validity.
use super::{StructuralJudge, StructuralTerm};

impl StructuralJudge<'_> {
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
