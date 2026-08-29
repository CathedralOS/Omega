//! Obligation-free total scalar identities.
//!
//! [`shapes`] owns the exact ordered identity vocabulary; [`rule`] owns
//! analysis admission, custody accounting, and candidate construction.

mod rule;
mod shapes;

pub use rule::WrappingNeutralArithmeticIdentityRule;
pub(in crate::rules::passes) use shapes::*;
