//! Obligation-free total scalar identities.
//!
//! [`rule`] owns each exact rule contract, [`shapes`] owns the wrapping
//! partitions, [`saturating`] owns its closed neutral-arithmetic partition,
//! and [`proposal`] owns their common analysis-to-candidate conveyor.

mod proposal;
mod rule;
mod saturating;
mod shapes;

pub use rule::{
    SaturatingNeutralArithmeticIdentityRule, WrappingMultiplyZeroAnnihilationRule,
    WrappingNeutralArithmeticIdentityRule, WrappingShiftZeroCountIdentityRule,
};
pub(in crate::rules::passes) use saturating::*;
pub(in crate::rules::passes) use shapes::*;
