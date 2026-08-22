//! Fixed affine-witness candidate frontier for independent reconstruction.

use std::collections::BTreeMap;

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{IntegerAffineWitness, check_integer_affine_witness};

/// Invocation-local source-order index of semantic rows that can extend an
/// affine definition chain from one exact value.
///
/// The index is candidate pruning only. Every indexed word is still replayed
/// independently by the proof kernel before it advances the frontier.
pub(crate) struct DefinitionIndex {
    by_input: BTreeMap<ScalarTerm, Vec<usize>>,
}

impl DefinitionIndex {
    pub(crate) fn new(semantic_axioms: &[Proposition]) -> Self {
        let mut by_input = BTreeMap::<ScalarTerm, Vec<usize>>::new();
        for (index, proposition) in semantic_axioms.iter().enumerate() {
            let Proposition::Equal(left, right) = proposition else {
                continue;
            };
            for (target, expression) in [(left, right), (right, left)] {
                if !matches!(target, ScalarTerm::Value { .. }) {
                    continue;
                }
                for input in affine_inputs(expression).into_iter().flatten() {
                    if !matches!(input, ScalarTerm::Value { .. }) {
                        continue;
                    }
                    let candidates = by_input.entry(input.clone()).or_default();
                    if candidates.last() != Some(&index) {
                        candidates.push(index);
                    }
                }
            }
        }
        Self { by_input }
    }

    fn candidates(&self, input: &ScalarTerm) -> &[usize] {
        self.by_input.get(input).map(Vec::as_slice).unwrap_or(&[])
    }
}

fn affine_inputs(expression: &ScalarTerm) -> [Option<&ScalarTerm>; 2] {
    match expression {
        ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerMultiply { left, right, .. } => {
            [Some(left.as_ref()), Some(right.as_ref())]
        }
        ScalarTerm::ExactIntegerSubtract { left, .. } => [Some(left.as_ref()), None],
        _ => [None, None],
    }
}

pub(super) fn definition_words(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
) -> Vec<Vec<usize>> {
    const MAX_DEFINITIONS: usize = 4;

    // This only prunes candidate words. Every retained prefix and final bound
    // is independently replayed by the reconstruction checkers.
    let mut words = Vec::new();
    let mut frontier = vec![(Vec::new(), 0, root.clone())];
    for _ in 0..MAX_DEFINITIONS {
        let mut next = Vec::new();
        for (prefix, start, current) in frontier {
            for &index in definitions
                .candidates(&current)
                .iter()
                .skip_while(|&&index| index < start)
            {
                let Proposition::Equal(left, right) = &semantic_axioms[index] else {
                    unreachable!("definition index contains only equality rows")
                };
                let mut word = prefix.clone();
                word.push(index);
                let next_target = [left, right]
                    .into_iter()
                    .filter(|target| matches!(target, ScalarTerm::Value { .. }))
                    .find(|target| {
                        check_integer_affine_witness(
                            context,
                            semantic_axioms,
                            &IntegerAffineWitness {
                                root: root.clone(),
                                target: (*target).clone(),
                                definition_axioms: word.clone(),
                            },
                        )
                        .is_ok()
                    });
                if let Some(next_target) = next_target {
                    words.push(word.clone());
                    next.push((word, index + 1, next_target.clone()));
                }
            }
        }
        frontier = next;
    }
    words
}
