//! Independent exact-cast definition-spine reconstruction.

use psi_core::{Proposition, ScalarTerm};

/// Reconstruct the unique exact-cast SSA definition spine.
///
/// This follows one definition per reached target and never explores alternate
/// paths or permutations. The proof-kernel witness checker still owns all cast
/// legality, continuity, and carrier validation.
pub(super) fn definition_axioms(
    root: &ScalarTerm,
    target: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Option<Vec<usize>> {
    if root == target {
        return None;
    }
    let mut current = target.clone();
    let mut reversed = Vec::new();
    while &current != root {
        if reversed.len() >= semantic_axioms.len() {
            return None;
        }
        let mut definitions = semantic_axioms
            .iter()
            .enumerate()
            .filter_map(|(index, axiom)| {
                let Proposition::Equal(output, ScalarTerm::IntegerExactCast { operand, .. }) =
                    axiom
                else {
                    return None;
                };
                (output == &current).then(|| (index, operand.as_ref().clone()))
            });
        let (index, operand) = definitions.next()?;
        if definitions.next().is_some() || reversed.contains(&index) {
            return None;
        }
        reversed.push(index);
        current = operand;
    }
    reversed.reverse();
    reversed
        .windows(2)
        .all(|pair| pair[0] < pair[1])
        .then_some(reversed)
}

/// Independently recover the unique non-cast source and first cast index.
pub(super) fn source_root(
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
                let Proposition::Equal(output, ScalarTerm::IntegerExactCast { operand, .. }) =
                    axiom
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
