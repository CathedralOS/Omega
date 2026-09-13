//! Retained affine-fact identity for certificate production.

use semantic_vocabulary::Proposition;

pub(super) fn distinct(left: &Proposition, right: &Proposition) -> bool {
    !std::ptr::eq(left, right)
}
