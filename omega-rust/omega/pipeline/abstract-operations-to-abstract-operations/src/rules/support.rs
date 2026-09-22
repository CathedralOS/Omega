//! Optimizer module role: stage group. Stable evidence and control-flow
//! concepts shared across named Psi passes: dominance and replacement
//! dominance (`control_flow`), frozen cyclic machines (`cycles`), the dead
//! scalar-node proposal two elimination passes share (`dead_scalar_node`),
//! accepted-obligation and literal-constant facts (`facts`), and the
//! provenance accounting of one elided node (`node_elision_accounting`).

mod control_flow;
mod cycles;
mod dead_scalar_node;
mod facts;
mod node_elision_accounting;

pub(super) use control_flow::{block_dominates, replacement_dominates_parameter_uses};
pub(super) use cycles::frozen_machines;
pub(super) use dead_scalar_node::{
    DeadScalarShape, propose_proof_certified_dead_scalar_nodes, propose_unproved_dead_scalar_nodes,
};
pub(super) use facts::{accepted_obligation_fact, boolean_constant, literal_integer_constant};
pub(super) use node_elision_accounting::node_elision_accounting;
