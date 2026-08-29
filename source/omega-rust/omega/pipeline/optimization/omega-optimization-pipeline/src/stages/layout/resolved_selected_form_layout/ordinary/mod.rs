//! Ordinary-function layout construction.
//!
//! Policy chooses the supported function family, order derives canonical block
//! order, plan assigns spans, function assembles rows, and row/branch own the
//! only byte decisions made at this stage.

mod branch;
mod function;
mod order;
mod plan;
mod policy;
mod row;

pub(super) use function::layout;
pub(super) use plan::instructions;
pub(super) use policy::select;
