//! Independent fixed-depth value-alias selection for obligation reconstruction.
//!
//! These selectors deliberately mirror, rather than share with, the untrusted
//! producer. Their one- and two-alias entry points expose no generic depth or
//! recursive graph search.

use psi_core::{Proposition, ScalarTerm};

mod cast;
mod index;
mod one;
mod two;

use index::distinct_same_carrier_values;

pub(super) use cast::{retained_landed_literal_cast, retained_stronger_cast};

pub(super) fn retained_one(
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    complete: impl FnMut(&ScalarTerm, &Proposition) -> bool,
) -> bool {
    one::retained(requirements, semantic_axioms, complete)
}

pub(super) fn retained_two(
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    complete: impl FnMut(&ScalarTerm, &Proposition) -> bool,
) -> bool {
    two::retained(requirements, semantic_axioms, complete)
}
