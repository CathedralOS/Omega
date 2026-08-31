//! Optimizer module role: stage group. Stable evidence and control-flow concepts shared across named Psi passes.

mod control_flow;
mod facts;
mod node_elision_accounting;

pub(in crate::rules::passes) use control_flow::{
    block_dominates, replacement_dominates_parameter_uses,
};
pub(in crate::rules::passes) use facts::{
    accepted_obligation_fact, boolean_constant, literal_integer_constant,
};
pub(in crate::rules::passes) use node_elision_accounting::node_elision_accounting;
