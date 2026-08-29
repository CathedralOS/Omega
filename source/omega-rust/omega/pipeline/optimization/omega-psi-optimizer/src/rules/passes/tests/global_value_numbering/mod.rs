//! Same-block, dominating, and phi-translated GVN coverage.
//!
//! The entrance supplies the shared pass-test vocabulary; each leaf owns one
//! recognizable GVN rule family.

use super::*;

mod compatible_policy;
mod dominating;
mod expression_vocabulary;
mod identities;
mod multiply_zero;
mod phi_translated;
mod saturating_neutral;
mod same_block;
