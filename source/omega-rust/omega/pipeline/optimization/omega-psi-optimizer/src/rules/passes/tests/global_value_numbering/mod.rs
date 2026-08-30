//! Optimizer module role: stage group. Same-block, dominating, and phi-translated GVN coverage.
//!
//! The entrance supplies the shared pass-test vocabulary; each leaf owns one
//! recognizable GVN rule family.

use super::*;

mod compatible_policy;
mod bitwise_absorbing;
mod bitwise_neutral;
mod dominating;
mod expression_vocabulary;
mod identities;
mod multiply_zero;
mod phi_translated;
mod saturating_neutral;
mod saturating_multiply_zero;
mod same_block;
