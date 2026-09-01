//! Optimizer module role: stage group. Same-block, dominating, and phi-translated GVN coverage.
//!
//! The entrance supplies the shared pass-test vocabulary; each leaf owns one
//! recognizable GVN rule family.

use super::*;

mod compatible_policy;
mod dominating;
mod expression_vocabulary;
mod phi_translated;
mod same_block;
mod total_scalar_identity;
