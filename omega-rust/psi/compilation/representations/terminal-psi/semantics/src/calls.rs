//! Calls: exact call-composition rows, runtime evidence calls, invocation
//! inputs, closed conformances and reach substitutions, and Fused service
//! binding shapes.

pub(crate) mod call_composition;
pub mod closed_conformances;
pub mod closed_reach;
pub mod evidence_calls;
pub(crate) mod invocation_inputs;
pub(crate) mod service_binding;
