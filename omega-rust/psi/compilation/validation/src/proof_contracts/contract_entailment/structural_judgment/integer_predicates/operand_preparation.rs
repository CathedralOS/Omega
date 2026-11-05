//! A batch asks about the same operands in several comparisons. Retain their
//! descriptions once while its judge is immutable. These are value descriptions,
//! not proofs or polynomial registrations: each goal still checks its own
//! operation tree and formation in a fresh arithmetic namespace.

use super::{
    ArithmeticDomain, IntegerObservations, PrimitiveType, StructuralJudge, StructuralTerm,
};

type CompletedCall = (StructuralTerm, StructuralTerm, PrimitiveType);

#[derive(Clone)]
pub(super) struct PreparedIntegerOperand {
    input: StructuralTerm,
    projection: Option<StructuralTerm>,
    completed_identity: Option<StructuralTerm>,
    // The outer option records whether classification ran; a negative answer
    // is useful too and must not trigger another scan of completed calls.
    completed_call: Option<Option<CompletedCall>>,
    observations: Vec<(PrimitiveType, Option<(StructuralTerm, ArithmeticDomain)>)>,
}

impl StructuralJudge<'_> {
    fn operand_preparation<'observations>(
        &self,
        value: &StructuralTerm,
        observations: &'observations mut IntegerObservations<'_>,
    ) -> Option<&'observations mut PreparedIntegerOperand> {
        let _profile = phase_profile::guard("psi.05.numeric.operands.lookup");
        // A dependency collector needs the actual reads, even if an earlier
        // question in this batch already computed the same description.
        if self.has_resolution_dependency_collector() {
            return None;
        }
        let operands = observations.operand_preparations.as_mut()?;
        let position = match operands
            .iter()
            .position(|operand| operand.input.same_resolution_payload(value))
        {
            Some(position) => position,
            None => {
                operands.push(PreparedIntegerOperand {
                    input: value.clone(),
                    projection: None,
                    completed_identity: None,
                    completed_call: None,
                    observations: Vec::new(),
                });
                operands.len() - 1
            }
        };
        Some(&mut operands[position])
    }

    pub(super) fn resolve_integer_operand(
        &self,
        value: StructuralTerm,
        observations: &mut IntegerObservations<'_>,
    ) -> StructuralTerm {
        let _profile = phase_profile::guard("psi.05.numeric.operands.resolve");
        let Some(operand) = self.operand_preparation(&value, observations) else {
            return self.integer_projection_value(value);
        };
        operand
            .projection
            .get_or_insert_with(|| self.integer_projection_value(value))
            .clone()
    }

    pub(super) fn prepared_completed_identity(
        &self,
        value: &StructuralTerm,
        observations: &mut IntegerObservations<'_>,
    ) -> StructuralTerm {
        let _profile = phase_profile::guard("psi.05.numeric.operands.identity");
        let Some(operand) = self.operand_preparation(value, observations) else {
            return self.completed_value_identity(value.clone());
        };
        operand
            .completed_identity
            .get_or_insert_with(|| self.completed_value_identity(value.clone()))
            .clone()
    }

    pub(super) fn prepared_completed_call(
        &self,
        value: &StructuralTerm,
        observations: &mut IntegerObservations<'_>,
    ) -> Option<CompletedCall> {
        let _profile = phase_profile::guard("psi.05.numeric.operands.call");
        let Some(operand) = self.operand_preparation(value, observations) else {
            return self.completed_integer_call(value);
        };
        operand
            .completed_call
            .get_or_insert_with(|| self.completed_integer_call(value))
            .clone()
    }

    pub(super) fn prepared_scalar_observation(
        &self,
        value: &StructuralTerm,
        primitive: PrimitiveType,
        observations: &mut IntegerObservations<'_>,
    ) -> Option<(StructuralTerm, ArithmeticDomain)> {
        let _profile = phase_profile::guard("psi.05.numeric.operands.observation");
        let Some(operand) = self.operand_preparation(value, observations) else {
            return self.completed_scalar_observation(value, primitive);
        };
        if let Some((_, answer)) = operand
            .observations
            .iter()
            .find(|(carrier, _)| *carrier == primitive)
        {
            return answer.clone();
        }
        let answer = self.completed_scalar_observation(value, primitive);
        operand.observations.push((primitive, answer.clone()));
        answer
    }
}
