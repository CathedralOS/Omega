//! Parameter lists: `parse_parameters` reads ordinary parameters and return
//! types, `parse_generic_parameters` the generic parameter syntax,
//! `binding_properties` the relevance brackets on a binding and `contracts`
//! the contracts attached to a machine parameter.

pub(crate) mod binding_properties;
pub(crate) mod contracts;
#[cfg(test)]
mod generic_parameters_tests;
pub(crate) mod parse_generic_parameters;
pub(crate) mod parse_parameters;
