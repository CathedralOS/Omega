//! Typing of declarations: data, machines, states, traits, operators,
//! measures, wire layouts, domains and their constraints. Each module lowers
//! one declaration form in the dependency order the route establishes.

pub(crate) mod data;
pub(crate) mod domain;
pub(crate) mod domain_constraints;
pub(crate) mod machine;
pub(crate) mod measure;
pub(crate) mod operator;
pub(crate) mod state;
pub(crate) mod trait_definition;
pub(crate) mod wire;
