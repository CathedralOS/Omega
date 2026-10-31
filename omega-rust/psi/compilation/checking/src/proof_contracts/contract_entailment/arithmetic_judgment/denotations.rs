//! Exact arithmetic value transport across independently prepared queries.
//! Opaque results carry their complete operand DAG in mint order. Transport
//! rebinds leaves simultaneously and uses the same operation constructors as
//! ordinary normalization; it imports neither old hypotheses nor authority.

use super::super::inductive_judgment::apply_argument_map;
use super::{Engine, OpaqueTerm, Polynomial};
use std::collections::{BTreeMap, BTreeSet};

/// A mathematical denotation, not a formation certificate. The consumer owns
/// the proof that this value exists and any bounds it retains after effects.
pub(in crate::proof_contracts::contract_entailment) struct IntegerDenotation {
    value: Polynomial,
    operations: Vec<(String, OpaqueTerm)>,
    leaves: Vec<String>,
}

impl IntegerDenotation {
    pub(in crate::proof_contracts::contract_entailment) fn leaf_atoms(&self) -> &[String] {
        &self.leaves
    }
}

impl OpaqueTerm {
    fn operands(&self) -> [Option<&Polynomial>; 2] {
        match self {
            Self::IntegerPolicy { value, .. } => [Some(value), None],
            Self::Remainder {
                operand, modulus, ..
            } => [Some(operand), Some(modulus)],
            Self::Quotient { dividend, divisor } => [Some(dividend), Some(divisor)],
            Self::ShiftLeft { value, count, .. } | Self::ShiftRight { value, count, .. } => {
                [Some(value), Some(count)]
            }
            Self::Bitwise { left, right, .. } => [Some(left), Some(right)],
        }
    }
}

impl Engine<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn capture_integer_denotation(
        &self,
        value: &Polynomial,
    ) -> Option<IntegerDenotation> {
        let value = self.substituted(value);
        let mut required: BTreeSet<String> = value
            .terms
            .keys()
            .flat_map(|monomial| monomial.keys().cloned())
            .collect();
        let mut operations = Vec::new();
        for (atom, operation) in self.opaque_terms.iter().rev() {
            if !required.remove(atom) {
                continue;
            }
            for operand in operation.operands().into_iter().flatten() {
                required.extend(
                    operand
                        .terms
                        .keys()
                        .flat_map(|monomial| monomial.keys().cloned()),
                );
            }
            operations.push((atom.clone(), operation.clone()));
        }
        // A forward/cyclic opaque dependency is not an observed leaf. Unknown
        // input cannot acquire a convenient identity by omission.
        if required
            .iter()
            .any(|atom| self.opaque_terms.iter().any(|(name, _)| name == atom))
        {
            return None;
        }
        operations.reverse();
        Some(IntegerDenotation {
            value,
            operations,
            leaves: required.into_iter().collect(),
        })
    }

    pub(in crate::proof_contracts::contract_entailment) fn instantiate_integer_denotation(
        &mut self,
        denotation: &IntegerDenotation,
        leaves: &BTreeMap<String, Polynomial>,
    ) -> Option<Polynomial> {
        let mut substitutions = leaves.clone();
        for (atom, operation) in &denotation.operations {
            let value = self.mapped_opaque_value(operation, &substitutions)?;
            substitutions.insert(atom.clone(), value);
        }
        apply_argument_map(&denotation.value, &substitutions)
    }

    /// Preserve complete dependencies on every transport, including a second
    /// arrival after the first re-mint. Missing leaves fail closed rather than
    /// retaining an atom from the old namespace.
    pub(in crate::proof_contracts::contract_entailment) fn extend_argument_map_over_opaque_terms(
        &mut self,
        substitutions: &mut BTreeMap<String, Polynomial>,
    ) {
        for index in 0..self.opaque_terms.len() {
            let (atom, operation) = self.opaque_terms[index].clone();
            if let Some(value) = self.mapped_opaque_value(&operation, substitutions) {
                substitutions.insert(atom, value);
            }
        }
    }

    fn mapped_opaque_value(
        &mut self,
        operation: &OpaqueTerm,
        substitutions: &BTreeMap<String, Polynomial>,
    ) -> Option<Polynomial> {
        let mapped = |value| apply_argument_map(value, substitutions);
        match operation {
            OpaqueTerm::IntegerPolicy {
                primitive,
                domain,
                value,
            } => self.integer_policy_value(*primitive, *domain, mapped(value)?),
            OpaqueTerm::Remainder {
                operand,
                modulus,
                tight_interval,
            } => self.integer_remainder(mapped(operand)?, mapped(modulus)?, *tight_interval),
            OpaqueTerm::Quotient { dividend, divisor } => {
                self.integer_quotient(mapped(dividend)?, mapped(divisor)?)
            }
            OpaqueTerm::ShiftLeft {
                value,
                count,
                count_bits,
            } => self.integer_shift_left(mapped(value)?, mapped(count)?, *count_bits),
            OpaqueTerm::ShiftRight {
                value,
                count,
                count_bits,
            } => self.integer_shift_right(mapped(value)?, mapped(count)?, *count_bits),
            OpaqueTerm::Bitwise {
                operator,
                left,
                right,
                bits,
                signed,
            } => self.integer_bitwise(*operator, mapped(left)?, mapped(right)?, *bits, *signed),
        }
    }
}
