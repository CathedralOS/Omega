//! Optimizer module role: stage group. Same-block, dominating, and phi-translated GVN coverage.
//!
//! The entrance supplies the shared pass-test vocabulary; each leaf owns one
//! recognizable GVN rule family.

use super::*;

mod expression_vocabulary;
mod scalar_common_subexpression;
mod total_scalar_identity;
