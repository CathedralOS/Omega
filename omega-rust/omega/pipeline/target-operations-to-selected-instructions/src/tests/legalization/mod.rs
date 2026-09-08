//! Optimizer module role: stage group. Legalization by admitted Unit and structural-call form.

pub(crate) mod byte_input;
mod byte_output;
mod claim_completion;
mod ieee_literal_sources;
mod installed_provider;
mod plain_unit;
mod primitive_stores;
mod process_exit;
mod projected_structural_call_return;
mod replay_corruption;
mod scalar_call_unit;
mod scalar_transfers;
mod scalar_unit_calls;
mod structural_call;
mod structural_publication;
mod unit_graph;
mod unit_view_graph;
mod widening;
