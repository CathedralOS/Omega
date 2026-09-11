//! Optimizer module role: stage group.
//! Common graph legalization and independent corruption controls.

pub(crate) mod byte_input;
mod byte_output;
mod ieee_literal_sources;
mod plain_unit;
mod primitive_stores;
mod process_exit;
mod scalar_arrays;
mod scalar_call_unit;
mod scalar_transfers;
mod scalar_unit_calls;
mod shared_type_catalog;
pub(crate) mod structural_case;
mod unit_graph;
mod unit_view_graph;
mod widening;
