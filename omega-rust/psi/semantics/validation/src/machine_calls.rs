//! Validating machine calls: call sites and their cycles, overloads and
//! result custody, invocation and machine-parameter admission, effects and
//! their inference, and the reference results a call returns.

pub(crate) mod call_cycles;
pub(crate) mod callable_overloads;
pub(crate) mod calls;
pub(crate) mod denotational_calls;
pub(crate) mod effect_inference;
pub(crate) mod effects;
pub(crate) mod fact_call_projections;
pub(crate) mod invocations;
pub(crate) mod machine_data;
pub(crate) mod machine_parameters;
pub(crate) mod machine_specialization_identity;
pub mod reference_result_custody;
pub(crate) mod result_overloads;
pub(crate) mod static_machine_call_contracts;
pub(crate) mod structural_call_custody;
