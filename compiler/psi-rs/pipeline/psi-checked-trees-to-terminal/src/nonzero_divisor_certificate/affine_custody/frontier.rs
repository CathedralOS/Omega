//! Fixed affine-witness candidate frontier for certificate production.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{IntegerAffineWitness, check_integer_affine_witness};

pub(super) fn definition_words(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
) -> Vec<Vec<usize>> {
    const MAX_DEFINITIONS: usize = 4;

    // This is candidate pruning, not proof authority: only prefixes replayed
    // successfully by the kernel advance, and the completed proof is checked
    // again before it leaves affine custody.
    let mut words = Vec::new();
    let mut frontier = vec![(Vec::new(), 0)];
    for _ in 0..MAX_DEFINITIONS {
        let mut next = Vec::new();
        for (prefix, start) in frontier {
            for index in start..semantic_axioms.len() {
                let Proposition::Equal(left, right) = &semantic_axioms[index] else {
                    continue;
                };
                let mut word = prefix.clone();
                word.push(index);
                let continues = [left, right]
                    .into_iter()
                    .filter(|target| matches!(target, ScalarTerm::Value { .. }))
                    .any(|target| {
                        check_integer_affine_witness(
                            context,
                            semantic_axioms,
                            &IntegerAffineWitness {
                                root: root.clone(),
                                target: target.clone(),
                                definition_axioms: word.clone(),
                            },
                        )
                        .is_ok()
                    });
                if continues {
                    words.push(word.clone());
                    next.push((word, index + 1));
                }
            }
        }
        frontier = next;
    }
    words
}
