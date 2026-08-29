//! Typed fixture catalog shared by stage-specific integration tests.

mod common;
mod control_flow;
mod selected_lowering;
mod structural_units;
mod validation;

pub(crate) use common::*;
pub(crate) use control_flow::*;
pub(crate) use selected_lowering::*;
pub(crate) use structural_units::*;
pub(crate) use validation::*;
