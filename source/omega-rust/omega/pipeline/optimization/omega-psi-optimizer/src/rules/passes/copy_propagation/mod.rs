//! Copy-propagation pass entrance.
//!
//! `catalog` owns exact rule order; `rule` owns proposal mechanics.

mod catalog;
mod rule;

pub(in crate::rules) use catalog::built_in_registrations;
pub use rule::RedundantBlockParameterRule;
