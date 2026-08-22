//! Source-ordered oriented equalities for independent affine-literal reconstruction.

use psi_core::{Proposition, ScalarTerm};

pub(super) fn ordered<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (&'a Proposition, &'a ScalarTerm, &'a ScalarTerm)> + 'a {
    requirements
        .iter()
        .chain(semantic_axioms)
        .filter_map(|equality| match equality {
            Proposition::Equal(left, right) => Some((equality, left, right)),
            _ => None,
        })
        .flat_map(|(equality, left, right)| [(equality, left, right), (equality, right, left)])
}
