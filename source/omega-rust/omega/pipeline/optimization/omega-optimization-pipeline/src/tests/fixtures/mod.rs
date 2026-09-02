//! Optimizer module role: stage group. Typed fixture catalog shared by stage-specific integration tests.

mod common;
mod control_flow;
mod projected_structural_call_return;
mod scalar_call_unit;
mod selected_lowering;
mod structural_units;
mod target_translation;
mod validation;

pub(crate) use common::*;
pub(crate) use control_flow::*;
pub(crate) use projected_structural_call_return::*;
pub(crate) use scalar_call_unit::*;
pub(crate) use selected_lowering::*;
pub(crate) use structural_units::*;
pub(crate) use target_translation::*;
pub(crate) use validation::*;
