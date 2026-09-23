//! One module per authored declaration form.
//!
//! `lower_symbol_resolved_trees` walks the resolved package one declaration
//! kind at a time and hands each to the module named after it. Every form
//! binds its interface through `crate::signatures`, its obligations through
//! `crate::contracts`, its written types through `crate::type_reference`, and
//! its bodies through `crate::expressions`; nothing here reaches back into
//! the entrance except through the shared `Lowerer`.

pub(crate) mod conformance;
#[cfg(test)]
mod conformance_tests;
pub(crate) mod data;
pub(crate) mod domain;
pub(crate) mod machine;
pub(crate) mod mathematical;
pub(crate) mod measure;
pub(crate) mod operator;
pub(crate) mod proposition;
pub(crate) mod state;
pub(crate) mod trait_definition;
pub(crate) mod wire;
