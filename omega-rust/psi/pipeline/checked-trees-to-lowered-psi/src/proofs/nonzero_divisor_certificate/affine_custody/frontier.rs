//! Fixed affine-witness candidate frontier for certificate production.

use std::rc::Rc;

use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

use super::DefinitionIndex;

mod layer;
mod prefix;

pub(super) fn definition_words(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
) -> Rc<[Vec<usize>]> {
    const MAX_DEFINITIONS: usize = 14;

    if let Some(words) = definitions.cached_words(root) {
        return words;
    }

    // This is candidate pruning, not proof authority: only prefixes replayed
    // successfully by the kernel advance, and the completed proof is checked
    // again before it leaves affine custody.
    let mut words = Vec::new();
    let mut frontier = vec![layer::Entry::root(root)];
    for depth in 0..MAX_DEFINITIONS {
        frontier = layer::expand(
            context,
            semantic_axioms,
            definitions,
            root,
            frontier,
            &mut words,
            depth + 1 < MAX_DEFINITIONS,
        );
        if frontier.is_empty() {
            break;
        }
    }
    let words = Rc::from(words);
    definitions.cache_words(root, Rc::clone(&words));
    words
}

pub(in crate::proofs::nonzero_divisor_certificate) fn definition_words_to_target(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    target: &ScalarTerm,
) -> Rc<[Vec<usize>]> {
    if let Some(words) = definitions.cached_words_to_target(root, target) {
        return words;
    }
    let words = definition_words(context, semantic_axioms, definitions, root)
        .iter()
        .filter(|word| {
            word.last().is_some_and(|&index| {
                let Proposition::Equal(left, right) = &semantic_axioms[index] else {
                    unreachable!("definition words contain only equality rows")
                };
                // A word can end at an endpoint or, traversing an unsigned
                // wrapping add or an exact add backward, at one of its
                // operand values.
                left == target
                    || right == target
                    || add_operand(left, target)
                    || add_operand(right, target)
            })
        })
        .cloned()
        .collect::<Rc<[Vec<usize>]>>();
    definitions.cache_words_to_target(root, target, Rc::clone(&words));
    words
}

fn add_operand(side: &ScalarTerm, target: &ScalarTerm) -> bool {
    let (ScalarTerm::WrappingIntegerAdd { left, right, .. }
    | ScalarTerm::ExactIntegerAdd { left, right, .. }) = side
    else {
        return false;
    };
    left.as_ref() == target || right.as_ref() == target
}

pub(in crate::proofs::nonzero_divisor_certificate) fn literal_axioms(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    definition_axioms: &[usize],
    target: &ScalarTerm,
) -> Option<Vec<Option<usize>>> {
    if let Some(literal_axioms) = definitions.cached_literal_axioms(root, definition_axioms, target)
    {
        return literal_axioms;
    }
    let literal_axioms =
        prefix::literal_axioms(context, semantic_axioms, root, definition_axioms, target);
    definitions.cache_literal_axioms(root, definition_axioms, target, literal_axioms.clone());
    literal_axioms
}
