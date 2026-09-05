//! Producer-local completion of affine witnesses for one exact target.

use proof_admission::IntegerAffineWitness;
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

use super::super::{DefinitionIndex, frontier};

pub(super) fn find<T>(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    definition_words: &[Vec<usize>],
    root: &ScalarTerm,
    target: &ScalarTerm,
    complete: &mut impl FnMut(IntegerAffineWitness) -> Option<T>,
) -> Option<T> {
    find_where(
        context,
        semantic_axioms,
        definitions,
        definition_words,
        root,
        target,
        |_| true,
        complete,
    )
}

pub(super) fn find_where<T>(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    definition_words: &[Vec<usize>],
    root: &ScalarTerm,
    target: &ScalarTerm,
    admit: impl Fn(&[usize]) -> bool,
    complete: &mut impl FnMut(IntegerAffineWitness) -> Option<T>,
) -> Option<T> {
    definition_words.iter().find_map(|definition_axioms| {
        if !admit(definition_axioms) {
            return None;
        }
        let literal_axioms = frontier::literal_axioms(
            context,
            semantic_axioms,
            definitions,
            root,
            definition_axioms,
            target,
        )?;
        complete(IntegerAffineWitness {
            root: root.clone(),
            target: target.clone(),
            literal_axioms,
            definition_axioms: definition_axioms.clone(),
        })
    })
}
