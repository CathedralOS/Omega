//! The interface vocabulary shared across declaration forms.
//!
//! Type parameters, state parameters and callable signatures are written the
//! same way wherever they appear, so they are typed once here rather than per
//! form: data, domains, machines, states, traits, operators, mathematical
//! definitions, propositions and conformances all reach this area.
//! `crate::declarations` is the caller; the crate entrance never invokes this
//! area directly.

pub(crate) mod callable_signature;
pub(crate) mod parameters;
#[cfg(test)]
mod type_parameter_tests;
pub(crate) mod type_parameters;
