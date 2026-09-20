//! Deterministic control-flow edge fact reconstruction.

use std::{
    collections::{BTreeMap, HashMap},
    hash::{Hash, Hasher},
};

use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ValueId};

use super::super::substitution::{
    proposition_mentions_substituted_value, substitute_proposition_values,
};

mod conditions;
mod discrete;
mod transport;
pub(super) use conditions::condition_fact;
pub(super) use transport::RewrittenSuccessorFact;

/// Bind one successor edge's parameters and restate the established facts
/// that mention its arguments.
///
/// The pushed `parameter = argument` equalities are licensed premise
/// introductions under `fact:successor-parameter-binding`. Each rewritten
/// established fact is discharged by the fixed-shape transport certificate in
/// [`transport`] before it joins the roster; the returned classification marks
/// certified emissions for `fact:successor-path-transport` and leaves rejected
/// certificates under the binding row's license. Production callers discard
/// the classification; only tests read it back.
pub(super) fn bind_successor_axioms(
    axioms: &mut Vec<Proposition>,
    target_block: &terminal_psi::Block,
    arguments: &[ValueId],
    value_term: &impl Fn(ValueId) -> ScalarTerm,
    proposition_context: &PropositionContext,
    rewrite_path_facts: bool,
) -> Vec<RewrittenSuccessorFact> {
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
    let mut rewritten_facts = Vec::new();
    if rewrite_path_facts {
        // The binding equalities just pushed occupy a contiguous run cited by
        // index inside each rewrite's transport certificate; the zip pushes
        // one per pair even when duplicate arguments collapse in the map.
        let equality_count = target_block.parameters.len().min(arguments.len());
        let equality_indices =
            (established.len()..established.len() + equality_count).collect::<Vec<_>>();
        let mut seen = axioms.iter().enumerate().fold(
            HashMap::<_, Vec<_>>::new(),
            |mut seen, (index, axiom)| {
                seen.entry(fact_cache_fingerprint(axiom))
                    .or_default()
                    .push(index);
                seen
            },
        );
        for (index, proposition) in established.iter().enumerate() {
            if !proposition_mentions_substituted_value(proposition, &substitutions) {
                continue;
            }
            let rewritten = substitute_proposition_values(proposition, &substitutions);
            let fingerprint = fact_cache_fingerprint(&rewritten);
            let duplicate = seen
                .get(&fingerprint)
                .is_some_and(|indices| indices.iter().any(|index| axioms[*index] == rewritten));
            if !duplicate {
                let certified = transport::successor_rewrite_certified(
                    proposition_context,
                    axioms,
                    index,
                    &equality_indices,
                    &rewritten,
                );
                seen.entry(fingerprint).or_default().push(axioms.len());
                axioms.push(rewritten.clone());
                rewritten_facts.push(RewrittenSuccessorFact {
                    proposition: rewritten,
                    certified,
                });
            }
        }
    }
    rewritten_facts
}

/// Append one already-established fact to a successor's roster together with
/// its restated form.
///
/// `proposition` itself joins by `push_unique` under whichever row licensed
/// its own introduction. The substituted copy follows it only when absent,
/// discharged by the same fixed-shape transport certificate
/// `bind_successor_axioms` uses; the returned classification marks whether the
/// certificate checker accepted that rewrite.
pub(super) fn append_successor_fact(
    axioms: &mut Vec<Proposition>,
    proposition: &Proposition,
    target_block: &terminal_psi::Block,
    arguments: &[ValueId],
    value_term: &impl Fn(ValueId) -> ScalarTerm,
    proposition_context: &PropositionContext,
) -> Vec<RewrittenSuccessorFact> {
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
    if axioms.contains(&rewritten) {
        return Vec::new();
    }
    // The source fact sits at its already-pushed roster position; this edge's
    // binding equalities are the `parameter = argument` axioms the successor
    // binding pushed before any rewrite.
    let source_index = axioms.iter().position(|axiom| axiom == proposition);
    let equality_indices = target_block
        .parameters
        .iter()
        .zip(arguments)
        .filter_map(|(parameter, argument)| {
            let equality = Proposition::Equal(value_term(parameter.id), value_term(*argument));
            axioms.iter().position(|axiom| axiom == &equality)
        })
        .collect::<Vec<_>>();
    let certified = source_index.is_some_and(|source_index| {
        transport::successor_rewrite_certified(
            proposition_context,
            axioms,
            source_index,
            &equality_indices,
            &rewritten,
        )
    });
    let fact = RewrittenSuccessorFact {
        proposition: rewritten.clone(),
        certified,
    };
    axioms.push(rewritten);
    vec![fact]
}

fn append_discrete_fact(propositions: &mut Vec<Proposition>, proposition: &Proposition) {
    if let Some(discrete) = discrete::strict_bound(proposition) {
        push_unique(propositions, discrete);
    }
    if let Some(bounds) = discrete::strict_carrier_bounds(proposition) {
        for bound in bounds {
            push_unique(propositions, bound);
        }
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
