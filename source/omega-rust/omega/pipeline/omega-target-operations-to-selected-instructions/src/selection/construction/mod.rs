//! Canonical selected-plan construction.

mod plan;
mod scalar;

pub(super) use plan::{build_plan, structural_call_row, structural_unit_layout};
