//! Retained affine-fact identity for certificate production.

use psi_core::Proposition;

pub(super) fn distinct(left: &Proposition, right: &Proposition) -> bool {
    !std::ptr::eq(left, right)
}
