//! Deterministic control-flow edge fact reconstruction.

use std::{
    collections::{BTreeMap, HashMap},
    hash::{Hash, Hasher},
};

use psi_core::{Proposition, ScalarTerm, ValueId};

use super::super::substitution::{
    proposition_mentions_substituted_value, substitute_proposition_values,
};

mod conditions;
mod discrete;
pub(super) use conditions::condition_fact;

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
                seen.entry(fact_cache_fingerprint(axiom))
                    .or_default()
                    .push(index);
                seen
            },
        );
        for proposition in &established {
            if !proposition_mentions_substituted_value(proposition, &substitutions) {
                continue;
            }
            let rewritten = substitute_proposition_values(proposition, &substitutions);
            let fingerprint = fact_cache_fingerprint(&rewritten);
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
) {
    let substitutions = target_block
        .parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (*argument, value_term(parameter.id)))
        .collect::<BTreeMap<_, _>>();
    push_unique(axioms, proposition.clone());
    let rewritten = substitute_proposition_values(proposition, &substitutions);
    append_discrete_fact(axioms, proposition);
    append_discrete_fact(axioms, &rewritten);
    push_unique(axioms, rewritten);
}

fn append_discrete_fact(propositions: &mut Vec<Proposition>, proposition: &Proposition) {
    if let Some(discrete) = discrete::strict_bound(proposition) {
        push_unique(propositions, discrete);
    }
}

fn push_unique(propositions: &mut Vec<Proposition>, proposition: Proposition) {
    if !propositions.contains(&proposition) {
        propositions.push(proposition);
    }
}

/// Fast, non-authoritative bucketing for exact proposition deduplication.
/// Collisions are always resolved with full proposition equality.
fn fact_cache_fingerprint(proposition: &Proposition) -> u64 {
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
        let (chunks, remainder) = bytes.as_chunks::<8>();
        for chunk in chunks {
            self.mix(u64::from_ne_bytes(*chunk));
        }
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
