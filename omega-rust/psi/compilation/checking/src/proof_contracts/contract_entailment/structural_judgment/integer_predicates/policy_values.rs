//! Project completed policy arithmetic after independent premises are live.
//!
//! The ordinary opaque representative is always available. Exploring its
//! computation happens in a private fork, so a declined projection or an
//! unformed Exact child cannot impose a new gate on that completed value.
//! Only a formed computation donates its policy-completed equation; the
//! existing integer owner decides reduction, clamping and resulting intervals.

use super::{
    BinaryOperator, Engine, IntegerObservations, Polynomial, StructuralJudge, StructuralTerm,
};

#[derive(Clone)]
pub(super) struct ObservedPolicyValue {
    source: StructuralTerm,
    representative: Polynomial,
    expanded: bool,
}

impl<'program> IntegerObservations<'program> {
    pub(super) fn defer_policy_value(&mut self, position: usize, representative: Polynomial) {
        let source = &self.atoms[position];
        if crate::proof_contracts::contract_entailment::struct_trace() {
            eprintln!("STRUCT policy-operand source={source:?}");
        }
        let StructuralTerm::ScalarBinary {
            operator, meaning, ..
        } = source
        else {
            return;
        };
        if Engine::integer_policy_operation_carrier(*operator, *meaning).is_some()
            && !self
                .policy_values
                .iter()
                .any(|known| known.source == *source)
        {
            self.policy_values.push(ObservedPolicyValue {
                source: source.clone(),
                representative,
                expanded: false,
            });
        }
    }

    pub(super) fn expand_policy_values(&mut self, judge: &StructuralJudge<'_>) -> Option<bool> {
        let mut changed = false;
        for index in 0..self.policy_values.len() {
            if self.policy_values[index].expanded {
                continue;
            }
            let pending = self.policy_values[index].clone();
            let mut candidate = self.clone();
            candidate.policy_values[index].expanded = true;
            let Some(value) = completed_value(judge, &pending.source, &mut candidate) else {
                if crate::proof_contracts::contract_entailment::struct_trace() {
                    eprintln!(
                        "STRUCT policy-projection-declined source={:?}",
                        pending.source
                    );
                }
                continue;
            };
            if crate::proof_contracts::contract_entailment::struct_trace() {
                eprintln!(
                    "STRUCT policy-projected source={:?} value={value:?}",
                    pending.source
                );
            }
            if !candidate.engine.install_hypotheses(vec![(
                BinaryOperator::Equal,
                pending.representative,
                value,
            )]) {
                return None;
            }
            *self = candidate;
            changed = true;
        }
        Some(changed)
    }
}

fn completed_value<'program>(
    judge: &StructuralJudge<'_>,
    source: &StructuralTerm,
    observations: &mut IntegerObservations<'program>,
) -> Option<Polynomial> {
    let StructuralTerm::ScalarBinary {
        references,
        operator,
        meaning,
        left,
        right,
    } = source
    else {
        return None;
    };
    let primitive = Engine::integer_policy_operation_carrier(*operator, *meaning)?;
    if ![left, right].into_iter().zip(references).all(|(operand, reference)| {
        crate::proof_contracts::contract_entailment::structural_terms::scalar_operand_is_observed(
            judge.program,
            *reference,
            operand,
        )
    }) {
        return None;
    }
    let start = observations.operations.len();
    let left = judge.integer_value(left, meaning[0].0, observations, 0)?;
    let right = judge.integer_value(right, meaning[1].0, observations, 0)?;
    if !observations.admit_formed_premises(&mut Vec::new())
        || !observations.admitted(start..observations.operations.len())
    {
        return None;
    }
    let value = observations
        .engine
        .integer_binary_value(*operator, left, right)?;
    observations
        .engine
        .integer_policy_value(primitive, meaning[0].1, value)
}
