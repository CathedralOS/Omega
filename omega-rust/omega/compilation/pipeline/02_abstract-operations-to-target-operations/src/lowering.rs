//! Optimizer module role: lowering modules. The executable entrance and the
//! retained owner live in `optimized.rs`; `coordination.rs` binds every
//! admitted input and lowers each function through the common control graph.

mod borrowed_storage;
mod boundary_settlements;
mod control_flow;
mod coordination;
pub(crate) mod descriptor_input_guard;
mod function;
mod function_signature;
pub(crate) mod model;
pub(crate) mod optimized;
pub mod placed_view_inputs;
mod provider_evidence;
mod scalar;
mod scalar_abi;
pub(crate) mod selected_boundary_calls;
pub(super) mod structural_layout;
pub(crate) mod structural_signature;
mod structural_type_lookup;
mod unit;
