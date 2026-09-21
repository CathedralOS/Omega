//! Evaluating closed integer facts at build time: domain facts,
//! const-generic calls and expressions, initializers, fixed-array lengths,
//! and named range arguments and endpoints.

pub(crate) mod const_applications;
pub(crate) mod const_domain_facts;
pub(crate) mod const_generic_calls;
pub(crate) mod const_generic_expressions;
pub(crate) mod const_initializers;
pub(crate) mod const_lengths;
pub(crate) mod range_arguments;
pub(crate) mod range_endpoints;
