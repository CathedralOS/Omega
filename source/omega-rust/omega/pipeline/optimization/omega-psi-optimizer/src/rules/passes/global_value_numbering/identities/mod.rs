//! Obligation-free total scalar identities.
//!
//! [`rule`] owns each exact rule contract, [`shapes`] owns the disjoint
//! arithmetic and shift partitions, and [`proposal`] owns their common
//! analysis-to-candidate conveyor.

mod proposal;
mod rule;
mod shapes;

pub use rule::{WrappingNeutralArithmeticIdentityRule, WrappingShiftZeroCountIdentityRule};
pub(in crate::rules::passes) use shapes::*;
