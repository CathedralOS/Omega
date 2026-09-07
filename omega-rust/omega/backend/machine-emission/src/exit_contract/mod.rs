//! Optimizer module role: stage group. Whole-function exit-contract staging, replay validation, and canonical identity.

mod compute;
mod error;
mod identity;
mod layout_optimization;
mod model;
mod stage;
mod validation;
mod validation_rules;

pub use error::*;
pub use layout_optimization::*;
pub use model::*;
pub use stage::*;
