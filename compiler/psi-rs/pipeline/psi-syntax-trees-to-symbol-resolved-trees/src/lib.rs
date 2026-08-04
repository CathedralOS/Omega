#![forbid(unsafe_code)]

//! Resolves names in parsed Omega source into stable Psi symbol identities.

mod constant;
mod data;
mod domain;
mod domain_establishment;
mod domain_operator_homes;
mod expression;
mod invariant;
mod item;
mod lowerer;
mod machine;
mod measure;
mod name;
mod operator;
mod proposition;
mod service_reaches;
mod state;
mod statement;
mod symbols;
mod trait_definition;
mod type_reference;
mod wire;

pub use lowerer::{lower_syntax_trees, lower_syntax_trees_with_sources};
