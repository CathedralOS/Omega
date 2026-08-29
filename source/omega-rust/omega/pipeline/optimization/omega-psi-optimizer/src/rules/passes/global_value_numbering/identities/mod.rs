//! Obligation-free total scalar identities.
//!
//! [`rule`] owns each exact rule contract, [`shapes`] owns the wrapping
//! partitions, [`saturating`] owns its closed neutral-arithmetic partition,
//! [`bitwise_neutral`] and [`bitwise_absorbing`] own separate exact-width
//! representation identities, and [`proposal`] owns their common
//! analysis-to-candidate conveyor.

mod bitwise_absorbing;
mod bitwise_neutral;
mod proposal;
mod rule;
mod saturating;
mod saturating_multiply_zero;
mod shapes;

pub use rule::{
    BitwiseAbsorbingLiteralIdentityRule, BitwiseNeutralLiteralIdentityRule,
    SaturatingMultiplyZeroAnnihilationRule, SaturatingNeutralArithmeticIdentityRule,
    WrappingMultiplyZeroAnnihilationRule, WrappingNeutralArithmeticIdentityRule,
    WrappingShiftZeroCountIdentityRule,
};
pub(in crate::rules::passes) use bitwise_absorbing::*;
pub(in crate::rules::passes) use bitwise_neutral::*;
pub(in crate::rules::passes) use saturating::*;
pub(in crate::rules::passes) use saturating_multiply_zero::*;
pub(in crate::rules::passes) use shapes::*;
