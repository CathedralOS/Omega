//! Producer-local discovery of the unique non-cast source.

use psi_core::{Proposition, ScalarTerm};

pub(super) fn root(
    target: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Option<(ScalarTerm, usize)> {
    let mut current = target.clone();
    let mut reversed = Vec::new();
    loop {
        if reversed.len() >= semantic_axioms.len() {
            return None;
        }
        let mut definitions = semantic_axioms
            .iter()
            .enumerate()
            .filter_map(|(index, axiom)| {
                let Proposition::Equal(
                    output,
                    ScalarTerm::IntegerExactCast { operand, .. }
                    | ScalarTerm::IntegerWiden { operand, .. },
                ) = axiom
                else {
                    return None;
                };
                (output == &current).then(|| (index, operand.as_ref().clone()))
            });
        let Some((index, operand)) = definitions.next() else {
            break;
        };
        if definitions.next().is_some() || reversed.contains(&index) {
            return None;
        }
        reversed.push(index);
        current = operand;
    }
    reversed.reverse();
    if reversed.is_empty() || !reversed.windows(2).all(|pair| pair[0] < pair[1]) {
        return None;
    }
    Some((current, reversed[0]))
}
