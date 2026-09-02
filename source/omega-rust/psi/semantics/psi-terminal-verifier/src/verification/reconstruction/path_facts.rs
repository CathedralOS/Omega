//! Deterministic control-flow edge fact reconstruction.

use std::{
    collections::{BTreeMap, HashMap},
    hash::{Hash, Hasher},
};

use psi_core::{IntegerSign, IntegerValue, Proposition, ScalarTerm, ValueId};

use super::super::substitution::{
    proposition_mentions_substituted_value, substitute_proposition_values,
    substitute_scalar_term_values,
};

pub(super) fn true_condition_fact(
    condition: ValueId,
    axioms: &[Proposition],
    value_term: &impl Fn(ValueId) -> ScalarTerm,
) -> Option<Proposition> {
    let condition = value_term(condition);
    let predicate = axioms.iter().rev().find_map(|axiom| match axiom {
        Proposition::Equal(left, right) if left == &condition => Some(right),
        Proposition::Equal(left, right) if right == &condition => Some(left),
        _ => None,
    })?;
    let constants = axioms
        .iter()
        .filter_map(|axiom| match axiom {
            Proposition::Equal(ScalarTerm::Value { id, .. }, value)
                if matches!(value, ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. }) =>
            {
                Some((*id, value.clone()))
            }
            Proposition::Equal(value, ScalarTerm::Value { id, .. })
                if matches!(value, ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. }) =>
            {
                Some((*id, value.clone()))
            }
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let predicate = substitute_scalar_term_values(predicate, &constants);
    match &predicate {
        ScalarTerm::BooleanEqual { left, right } | ScalarTerm::IntegerEqual { left, right, .. } => {
            Some(Proposition::Equal((**left).clone(), (**right).clone()))
        }
        ScalarTerm::IntegerLessThan { left, right, .. } => {
            Some(Proposition::LessThan((**left).clone(), (**right).clone()))
        }
        ScalarTerm::IntegerLessOrEqual { left, right, .. } => Some(Proposition::LessOrEqual(
            (**left).clone(),
            (**right).clone(),
        )),
        _ => None,
    }
}

pub(super) fn bind_successor_axioms(
    axioms: &mut Vec<Proposition>,
    target_block: &psi_terminal::Block,
    arguments: &[ValueId],
    value_term: &impl Fn(ValueId) -> ScalarTerm,
    rewrite_path_facts: bool,
) {
    let substitutions = target_block
        .parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (*argument, value_term(parameter.id)))
        .collect::<BTreeMap<_, _>>();
    let established = axioms.clone();
    // Emit edge equalities before rewritten path facts so independently
    // reconstructed axiom indexes are deterministic.
    for (parameter, argument) in target_block.parameters.iter().zip(arguments) {
        axioms.push(Proposition::Equal(
            value_term(parameter.id),
            value_term(*argument),
        ));
    }
    if rewrite_path_facts {
        let mut seen = axioms.iter().enumerate().fold(
            HashMap::<_, Vec<_>>::new(),
            |mut seen, (index, axiom)| {
                seen.entry(fact_fingerprint(axiom)).or_default().push(index);
                seen
            },
        );
        for proposition in &established {
            if !proposition_mentions_substituted_value(proposition, &substitutions) {
                continue;
            }
            let rewritten = substitute_proposition_values(proposition, &substitutions);
            let fingerprint = fact_fingerprint(&rewritten);
            let duplicate = seen
                .get(&fingerprint)
                .is_some_and(|indices| indices.iter().any(|index| axioms[*index] == rewritten));
            if !duplicate {
                seen.entry(fingerprint).or_default().push(axioms.len());
                axioms.push(rewritten);
            }
        }
    }
}

pub(super) fn append_successor_fact(
    axioms: &mut Vec<Proposition>,
    proposition: &Proposition,
    target_block: &psi_terminal::Block,
    arguments: &[ValueId],
    value_term: &impl Fn(ValueId) -> ScalarTerm,
    derive_discrete_unsigned_positive: bool,
) {
    let substitutions = target_block
        .parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (*argument, value_term(parameter.id)))
        .collect::<BTreeMap<_, _>>();
    push_unique(axioms, proposition.clone());
    let rewritten = substitute_proposition_values(proposition, &substitutions);
    if derive_discrete_unsigned_positive {
        append_discrete_unsigned_positive_fact(axioms, proposition);
        append_discrete_unsigned_positive_fact(axioms, &rewritten);
    }
    push_unique(axioms, rewritten);
}

/// Fixed unsigned integers are discrete: a taken `0 < value` edge also
/// establishes the exact `1 <= value` premise needed by subtraction of one.
fn append_discrete_unsigned_positive_fact(
    propositions: &mut Vec<Proposition>,
    proposition: &Proposition,
) {
    let Proposition::LessThan(
        ScalarTerm::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(0),
        },
        right,
    ) = proposition
    else {
        return;
    };
    if scalar_type.sign() != IntegerSign::Unsigned {
        return;
    }
    let one = ScalarTerm::integer(*scalar_type, IntegerValue::Unsigned(1))
        .expect("one belongs to every validated unsigned fixed carrier");
    push_unique(propositions, Proposition::LessOrEqual(one, right.clone()));
}

fn push_unique(propositions: &mut Vec<Proposition>, proposition: Proposition) {
    if !propositions.contains(&proposition) {
        propositions.push(proposition);
    }
}

/// Fast, non-authoritative bucketing for exact proposition deduplication.
/// Collisions are always resolved with full proposition equality.
fn fact_fingerprint(proposition: &Proposition) -> u64 {
    let mut hasher = FactHasher::default();
    proposition.hash(&mut hasher);
    hasher.finish()
}

#[derive(Default)]
struct FactHasher(u64);

impl FactHasher {
    fn mix(&mut self, value: u64) {
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

impl Hasher for FactHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            self.mix(u64::from_ne_bytes(chunk.try_into().expect("exact chunk")));
        }
        let remainder = chunks.remainder();
        if !remainder.is_empty() {
            let mut tail = [0_u8; 8];
            tail[..remainder.len()].copy_from_slice(remainder);
            self.mix(u64::from_ne_bytes(tail) ^ (remainder.len() as u64));
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.mix(value);
    }

    fn write_u128(&mut self, value: u128) {
        self.mix(value as u64);
        self.mix((value >> 64) as u64);
    }

    fn write_usize(&mut self, value: usize) {
        self.mix(value as u64);
    }

    fn write_isize(&mut self, value: isize) {
        self.mix(value as u64);
    }
}
