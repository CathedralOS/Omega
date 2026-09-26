//! Advancing the program: what the evaluator DOES, as opposed to what it
//! reads.
//!
//! `states_and_transitions` drives one machine's states and the edges between
//! them, and the rest are what a step can reach: `statements_and_calls` and
//! `expressions_and_value_calls` for authored code, `scalar_operations`,
//! `casts_and_recasts` and `numeric_landing` for scalar work, `host_dispatch`,
//! `boundary_adapter_dispatch` and `boundary_console` for leaving the program,
//! and `wire_codec` with `wire_verification` for the encoded form crossing
//! that edge.

pub(in crate::interpreter::evaluator) mod boundary_adapter_dispatch;
pub(in crate::interpreter::evaluator) mod boundary_console;
pub(in crate::interpreter::evaluator) mod casts_and_recasts;
pub(in crate::interpreter::evaluator) mod expressions_and_value_calls;
pub(in crate::interpreter::evaluator) mod host_dispatch;
pub(in crate::interpreter::evaluator) mod numeric_landing;
pub(in crate::interpreter::evaluator) mod scalar_operations;
pub(in crate::interpreter::evaluator) mod statements_and_calls;
pub(in crate::interpreter::evaluator) mod states_and_transitions;
pub(in crate::interpreter::evaluator) mod wire_codec;
pub(crate) mod wire_verification;
