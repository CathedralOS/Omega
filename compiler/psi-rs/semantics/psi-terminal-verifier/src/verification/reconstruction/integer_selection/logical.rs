//! Independent canonical compound integer proposition reconstruction.

use psi_core::Proposition;

pub(super) fn retained_conjunction(
    conjuncts: &[Proposition],
    mut retained: impl FnMut(&Proposition) -> bool,
) -> bool {
    !conjuncts.is_empty() && conjuncts.iter().all(&mut retained)
}

pub(super) fn retained_disjunction(
    disjuncts: &[Proposition],
    retained: impl FnMut(&Proposition) -> bool,
) -> bool {
    disjuncts.iter().any(retained)
}
