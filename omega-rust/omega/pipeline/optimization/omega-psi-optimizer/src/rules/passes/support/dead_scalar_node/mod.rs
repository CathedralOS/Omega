//! Optimizer module role: stage group. Shared traversal protocol for exact rules that remove one unused scalar node.

mod model;
mod proposal;

pub(in crate::rules::passes) use model::DeadScalarShape;
pub(in crate::rules::passes) use proposal::{
    propose_proof_certified_dead_scalar_nodes, propose_unproved_dead_scalar_nodes,
};
